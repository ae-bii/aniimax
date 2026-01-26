// Aniimax Web Application

import init, { optimize, get_version } from './pkg/aniimax.js';

let wasmReady = false;

// Initialize WASM module
async function initWasm() {
    try {
        await init();
        wasmReady = true;
        
        // Display version
        const version = get_version();
        document.getElementById('version').textContent = version;
        
        console.log(`Aniimax v${version} loaded successfully`);
    } catch (error) {
        console.error('Failed to initialize WASM:', error);
        showError('Failed to load the optimizer. Please refresh the page.');
    }
}

// Get input values from the form
function getInputValues() {
    return {
        target_amount: parseFloat(document.getElementById('target').value) || 1000,
        currency: document.getElementById('currency').value,
        energy_self_sufficient: document.getElementById('energy-self-sufficient').checked,
        parallel: document.getElementById('parallel-production').checked,
        energy_cost_per_min: parseFloat(document.getElementById('energy-cost').value) || 0,
        farmland: {
            count: parseInt(document.getElementById('farmland-count').value) || 1,
            level: parseInt(document.getElementById('farmland-level').value) || 1
        },
        woodland: {
            count: parseInt(document.getElementById('woodland-count').value) || 1,
            level: parseInt(document.getElementById('woodland-level').value) || 1
        },
        mineral_pile: {
            count: parseInt(document.getElementById('mineral-pile-count').value) || 1,
            level: parseInt(document.getElementById('mineral-pile-level').value) || 1
        },
        carousel_mill: {
            count: parseInt(document.getElementById('carousel-mill-count').value) || 1,
            level: parseInt(document.getElementById('carousel-mill-level').value) || 1
        },
        jukebox_dryer: {
            count: parseInt(document.getElementById('jukebox-dryer-count').value) || 1,
            level: parseInt(document.getElementById('jukebox-dryer-level').value) || 1
        },
        crafting_table: {
            count: parseInt(document.getElementById('crafting-table-count').value) || 1,
            level: parseInt(document.getElementById('crafting-table-level').value) || 1
        },
        dance_pad_polisher: {
            count: parseInt(document.getElementById('dance-pad-polisher-count').value) || 1,
            level: parseInt(document.getElementById('dance-pad-polisher-level').value) || 1
        },
        aniipod_maker: {
            count: parseInt(document.getElementById('aniipod-maker-count').value) || 1,
            level: parseInt(document.getElementById('aniipod-maker-level').value) || 1
        },
        modules: {
            ecological_module: parseInt(document.getElementById('ecological-module-level').value) || 0,
            kitchen_module: parseInt(document.getElementById('kitchen-module-level').value) || 0,
            mineral_detector: parseInt(document.getElementById('mineral-detector-level').value) || 0,
            crafting_module: parseInt(document.getElementById('crafting-module-level').value) || 0
        }
    };
}

// Format time for display
function formatTime(seconds) {
    const totalSecs = Math.floor(seconds);
    const hours = Math.floor(totalSecs / 3600);
    const minutes = Math.floor((totalSecs % 3600) / 60);
    const secs = totalSecs % 60;

    if (hours > 0) {
        return `${hours}h ${minutes}m ${secs}s`;
    } else if (minutes > 0) {
        return `${minutes}m ${secs}s`;
    } else {
        return `${secs}s`;
    }
}

// Format number with commas
function formatNumber(num) {
    return num.toLocaleString(undefined, { maximumFractionDigits: 2 });
}

// Show error message
function showError(message) {
    const errorEl = document.getElementById('error-message');
    const resultsContent = document.getElementById('results-content');
    const resultsSection = document.getElementById('results-section');
    
    errorEl.textContent = message;
    errorEl.style.display = 'block';
    resultsContent.style.display = 'none';
    resultsSection.style.display = 'block';
}

// Display results
function displayResults(result) {
    const resultsSection = document.getElementById('results-section');
    const errorEl = document.getElementById('error-message');
    const resultsContent = document.getElementById('results-content');
    
    resultsSection.style.display = 'block';
    
    if (!result.success) {
        showError(result.error || 'An unknown error occurred.');
        return;
    }
    
    errorEl.style.display = 'none';
    resultsContent.style.display = 'block';
    
    // Update summary
    document.getElementById('total-profit').textContent = 
        `${formatNumber(result.total_profit)} ${result.currency}`;
    document.getElementById('total-time').textContent = result.total_time_formatted;
    document.getElementById('total-energy').textContent = 
        result.total_energy !== null ? formatNumber(result.total_energy) : 'N/A';
    document.getElementById('items-produced').textContent = formatNumber(result.items_produced);
    
    // Update mode indicator
    const energySelfSufficient = document.getElementById('energy-self-sufficient').checked;
    const parallelMode = document.getElementById('parallel-production').checked;
    let modeText = 'time efficiency';
    if (result.is_energy_self_sufficient) {
        modeText = 'energy self-sufficient';
    } else if (parallelMode && result.steps && result.steps.length > 1) {
        modeText = 'cross-facility parallel';
    }
    document.getElementById('sort-criteria').textContent = modeText;
    
    // Display production steps
    const stepsList = document.getElementById('steps-list');
    stepsList.innerHTML = '';
    
    // Add energy self-sufficiency info if applicable
    if (result.is_energy_self_sufficient && result.energy_item_name) {
        const infoEl = document.createElement('div');
        infoEl.className = 'energy-info';
        infoEl.innerHTML = `
            <strong>Energy Self-Sufficient Mode</strong><br>
            Total: ${formatNumber(result.energy_items_produced || 0)} ${result.energy_item_name} consumed for energy
        `;
        stepsList.appendChild(infoEl);
    }

    // Add parallel production info if applicable
    if (parallelMode && result.steps && result.steps.length > 1) {
        const infoEl = document.createElement('div');
        infoEl.className = 'parallel-info';
        infoEl.innerHTML = `
            <strong>Cross-Facility Parallel Mode</strong><br>
            Running ${result.steps.length} facilities simultaneously. Total time = longest step.
        `;
        stepsList.appendChild(infoEl);
    }
    
    const isParallelResult = parallelMode && result.steps && result.steps.length > 1;
    
    result.steps.forEach((step, index) => {
        const stepEl = document.createElement('div');
        stepEl.className = 'step-item';
        const isEnergyStep = step.item_name.includes('(for energy)');
        const isProfitStep = step.item_name.includes('(for profit)');
        let stepClass = isEnergyStep ? 'energy-step' : (isProfitStep ? 'profit-step' : '');
        if (isParallelResult) stepClass = 'parallel-step';
        
        // For parallel mode, show "||" symbol; otherwise show step number
        const stepIndicator = isParallelResult ? '||' : (index + 1);
        
        stepEl.innerHTML = `
            <div class="step-number ${stepClass}">${stepIndicator}</div>
            <div class="step-details">
                <div class="step-name">${step.quantity} batches of ${step.item_name}</div>
                <div class="step-facility">at ${step.facility}</div>
            </div>
            <div class="step-meta">
                ${step.time_seconds > 0 ? `Time: ${formatTime(step.time_seconds)}` : ''}
                ${step.energy !== null && step.energy > 0 ? `<br>Energy: ${formatNumber(step.energy)}` : ''}
            </div>
        `;
        stepsList.appendChild(stepEl);
    });
    
    // Display all options table
    const tbody = document.getElementById('options-tbody');
    tbody.innerHTML = '';
    
    // Sort efficiencies by profit per second
    const sortedEfficiencies = [...result.all_efficiencies].sort((a, b) => {
        return b.profit_per_second - a.profit_per_second;
    });
    
    sortedEfficiencies.forEach(eff => {
        const row = document.createElement('tr');
        row.innerHTML = `
            <td>${eff.item_name}</td>
            <td>${eff.facility} (Lv.${eff.facility_level})</td>
            <td>${formatNumber(eff.profit_per_second)}</td>
            <td>${eff.profit_per_energy !== null ? formatNumber(eff.profit_per_energy) : 'N/A'}</td>
            <td>${formatTime(eff.total_time_per_unit)}</td>
        `;
        tbody.appendChild(row);
    });
    
    // Scroll to results
    resultsSection.scrollIntoView({ behavior: 'smooth' });
}

// Run optimization
async function runOptimization() {
    if (!wasmReady) {
        showError('Optimizer not ready. Please wait...');
        return;
    }
    
    const btn = document.getElementById('optimize-btn');
    const btnText = btn.querySelector('.btn-text');
    const btnLoading = btn.querySelector('.btn-loading');
    
    // Show loading state
    btn.disabled = true;
    btnText.style.display = 'none';
    btnLoading.style.display = 'inline';
    
    try {
        const input = getInputValues();
        const inputJson = JSON.stringify(input);
        
        // Run optimizer (async to not block UI)
        await new Promise(resolve => setTimeout(resolve, 10));
        const resultJson = optimize(inputJson);
        const result = JSON.parse(resultJson);
        
        displayResults(result);
    } catch (error) {
        console.error('Optimization error:', error);
        showError(`Optimization failed: ${error.message}`);
    } finally {
        // Reset button state
        btn.disabled = false;
        btnText.style.display = 'inline';
        btnLoading.style.display = 'none';
    }
}

// Event listeners
document.addEventListener('DOMContentLoaded', () => {
    initWasm();
    
    document.getElementById('optimize-btn').addEventListener('click', runOptimization);
    
    // Disable parallel checkbox when energy self-sufficient is checked
    const energySelfSufficientCheckbox = document.getElementById('energy-self-sufficient');
    const parallelCheckbox = document.getElementById('parallel-production');
    
    energySelfSufficientCheckbox.addEventListener('change', () => {
        parallelCheckbox.disabled = energySelfSufficientCheckbox.checked;
        if (energySelfSufficientCheckbox.checked) {
            parallelCheckbox.checked = false;
        }
    });
    
    // Allow Enter key to trigger optimization
    document.querySelectorAll('input').forEach(input => {
        input.addEventListener('keypress', (e) => {
            if (e.key === 'Enter') {
                runOptimization();
            }
        });
    });
});
