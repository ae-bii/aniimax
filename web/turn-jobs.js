// Assign recipes that may take turns to physical facility units of any type. Prefer a dedicated
// unit for each recipe when the player owns enough, then share only remaining workloads.
export function allocateTurnJobs(rows, owned) {
    const usable = rows
        .filter(row => row.cycle_time > 0)
        .map(row => ({
            item: row.item_name,
            cycle: row.cycle_time,
            busy: Math.max(0, row.busy_units ?? row.facility_count),
            units: Math.max(0, row.facility_count || 0),
        }))
        .filter(row => row.busy > 1e-9);
    if (owned <= 0) return [];
    if (!usable.length) return Array.from({ length: owned }, () => []);

    const totalBusy = usable.reduce((sum, row) => sum + row.busy, 0);
    const requested = usable.reduce((sum, row) => sum + row.units, 0);
    const count = Math.min(owned, Math.max(Math.ceil(totalBusy - 1e-6), requested));
    const machines = Array.from({ length: count }, () => ({ busy: 0, jobs: [] }));

    // Whole workloads need their own machines. Fractional workloads also get separate machines
    // while one is free, so multiple owned units can run separate recipes instead of showing one
    // unit with every recipe and the remaining units as idle.
    const fractions = [];
    let nextEmpty = 0;
    usable.forEach(row => {
        let remaining = row.busy;
        while (remaining >= 1 - 1e-6 && nextEmpty < machines.length) {
            machines[nextEmpty].busy = 1;
            machines[nextEmpty].jobs.push({ item: row.item, cycle: row.cycle, rate: 1 / row.cycle });
            nextEmpty++;
            remaining -= 1;
        }
        if (remaining > 1e-6) fractions.push({ ...row, busy: remaining });
    });
    fractions.forEach(row => {
        let remaining = row.busy;
        if (nextEmpty < machines.length) {
            machines[nextEmpty].busy = remaining;
            machines[nextEmpty].jobs.push({ item: row.item, cycle: row.cycle, rate: remaining / row.cycle });
            nextEmpty++;
            return;
        }
        for (const machine of machines) {
            const share = Math.min(remaining, 1 - machine.busy);
            if (share <= 1e-6) continue;
            machine.busy += share;
            machine.jobs.push({ item: row.item, cycle: row.cycle, rate: share / row.cycle });
            remaining -= share;
            if (remaining <= 1e-6) break;
        }
    });

    // Return every physical unit, including idle ones. The caller can then replace the plan's
    // original idle row instead of drawing it in addition to these redistributed active units.
    while (machines.length < owned) machines.push({ busy: 0, jobs: [] });
    return machines.map(machine => machine.jobs);
}

// Build the complete physical-unit allocation for every facility with a producing turn-capable
// recipe. All rows for those facilities are consumed here, including idle rows, so callers must
// not render the same rows again through their ordinary facility path.
export function allocateTurnFacilities(steps, ownedFor, takesTurns) {
    const facilities = new Set(steps.filter(takesTurns).map(step => step.facility));
    const rowsByFacility = new Map();
    steps.filter(step => facilities.has(step.facility)).forEach(step => {
        rowsByFacility.set(step.facility, [...(rowsByFacility.get(step.facility) || []), step]);
    });
    return new Map([...rowsByFacility].map(([facility, rows]) => [facility, allocateTurnJobs(rows, ownedFor(facility))]));
}

// Match the facility-plan table to the redistributed physical units. Producing recipe rows remain
// separate, but the solver's old idle row is removed or reduced to the units that are still idle
// after the recipes have been spread across the owned units.
export function redistributeTurnFacilityRows(steps, ownedFor, takesTurns) {
    const allocations = allocateTurnFacilities(steps, ownedFor, takesTurns);
    const idleByFacility = new Map([...allocations].map(([facility, units]) => [facility, units.filter(jobs => jobs.length === 0).length]));
    const idleShown = new Set();

    return steps.flatMap(step => {
        if (!allocations.has(step.facility) || step.status === 'producing') return [step];
        const idle = idleByFacility.get(step.facility) || 0;
        if (idle <= 0 || idleShown.has(step.facility)) return [];
        idleShown.add(step.facility);
        return [{ ...step, facility_count: idle }];
    });
}
