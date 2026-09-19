// Shared facility configuration, used by app.js for both the facility input cards and the
// facility recipe reference modal, so the two stay in sync automatically.

// Facility configuration. `name` must exactly match the facility string used throughout the
// Rust data model (ProductionItem.facility / FacilityCounts keys) since it's sent verbatim as
// the JSON key for each facility's count/level. Add new facilities here only; cards and input
// handling are generated dynamically, no other file needs to change. `category` groups the cards
// in the UI (see `FACILITY_CATEGORIES` below for display order). `hasLevels: false` hides the
// Level input entirely for facilities that don't level up in-game; omit the field (defaults to
// leveled) for any facility that does. `hasWorker: true` marks facilities an Aniimo works;
// `ability` is the Aniimo ability the facility uses and `personality` the personality that gets
// its +20% speed bonus (omitted if not known), shown in the plan's Aniimo recommendations.
// `unlocks` maps each facility level to the RV (Homeland) level that unlocks it. `counts[i]` is how
// many of the facility you can place at RV level i + 1; an RV level past the end of the list keeps
// the last count (the environment buildings' later counts aren't known yet). Simple mode uses both
// (see `simpleSetup`). Farmland, Woodland and Mine counts come from the game; every other facility
// is placed once, confirmed up to RV level 8.
//
// Facilities marked "Not yet verified in game" in their tooltip haven't had their numbers
// confirmed in game yet.
export const FACILITIES = [
    {
        name: 'Farmland', slug: 'farmland', defaultCount: 1, category: 'Materials',
        unlocks: { 1: 1, 2: 2, 3: 5, 4: 7, 5: 9, 6: 12, 7: 16 },
        counts: [4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30, 32, 34, 36, 38, 40, 42],
        tooltip: "Lv.1: wheat&#10;Lv.2: potato, quick wheat&#10;Lv.3: rice, soybean&#10;Lv.4: rose, cotton, quick potato&#10;Lv.5: strawberry, lavender, sugarcane&#10;Lv.6: ginseng, grape, premium wheat, quick rice&#10;Lv.7: cranberry, agave, quick strawberry"
    },
    {
        name: 'Woodland', slug: 'woodland', defaultCount: 1, category: 'Materials',
        unlocks: { 1: 2, 2: 4, 3: 7, 4: 11, 5: 14, 6: 18 },
        counts: [0, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21],
        tooltip: "Lv.1: willow wood&#10;Lv.2: bamboo, lemon&#10;Lv.3: cherry blossom, apple, maple syrup, quick bamboo&#10;Lv.4: palm bark, chestnut, walnut, quick lemon&#10;Lv.5: natural rubber, coconut, quick maple syrup&#10;Lv.6: cocoa, orange flower, quick coconut&#10;Also yields Wood Blocks"
    },
    {
        name: 'Mine', slug: 'mine', defaultCount: 1, category: 'Materials', hasWorker: true, ability: 'Earth', personality: 'Playful',
        unlocks: { 1: 3, 2: 6, 3: 9, 4: 12, 5: 15, 6: 18 },
        counts: [0, 0, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10],
        tooltip: "Lv.1: rock&#10;Lv.2: clay&#10;Lv.3: shell&#10;Lv.4: copper ore&#10;Lv.5: quartz ore&#10;Lv.6: gem&#10;Also yields Mineral Sand."
    },
    {
        name: 'Well', slug: 'well', defaultCount: 0, category: 'Materials', hasWorker: true, ability: 'Water', personality: 'Faithful',
        unlocks: { 1: 4, 2: 8, 3: 11, 4: 13, 5: 17 },
        counts: [0, 0, 0, 1, 1, 1, 1, 2],
        tooltip: "Lv.1: well water, quick well water&#10;Lv.2: fresh water&#10;Lv.3: quick fresh water&#10;Lv.4: deep rock spring water, quick deep rock spring water&#10;Lv.5: natural mineral spring water, quick natural mineral spring water"
    },
    {
        name: 'Tidewhisper Sandcastle', slug: 'tidewhisper-sandcastle', defaultCount: 0, category: 'Aniimo Materials', hasWorker: true, ability: 'Leisure', personality: 'Judicious',
        unlocks: { 1: 5, 2: 8, 3: 13 },
        counts: [0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        tooltip: "Lv.1: sea salt&#10;Lv.2: quick sea salt&#10;Lv.3: pearl (needs Warm)"
    },
    {
        name: 'Dewy House', slug: 'dewy-house', defaultCount: 0, category: 'Aniimo Materials', hasWorker: true, ability: 'Leisure', personality: 'Instinctive',
        unlocks: { 1: 6, 2: 11 },
        counts: [0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        tooltip: "Lv.1: aromathyst&#10;Lv.2: quick aromathyst"
    },
    {
        name: 'Nimbus Bed', slug: 'nimbus-bed', defaultCount: 0, category: 'Aniimo Materials', hasWorker: true, ability: 'Leisure', personality: 'Judicious',
        unlocks: { 1: 10, 2: 13, 3: 16 },
        counts: [0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        tooltip: "Lv.1: wool&#10;Lv.2: quick wool&#10;Lv.3: petals&#10;Not yet verified in game."
    },
    {
        name: 'Starfall Hammock', slug: 'starfall-hammock', defaultCount: 0, category: 'Aniimo Materials', hasLevels: false, hasWorker: true, ability: 'Leisure', personality: 'Faithful',
        unlocks: { 1: 12 },
        counts: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        tooltip: "star (needs Cool)&#10;Not yet verified in game."
    },
    {
        name: 'Floral Windmill', slug: 'floral-windmill', defaultCount: 0, category: 'Aniimo Materials', hasLevels: false, hasWorker: true, ability: 'Leisure', personality: 'Nimble',
        unlocks: { 1: 18 },
        counts: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1],
        tooltip: "scales, quick scales (need Adequate)&#10;Not yet verified in game."
    },
    {
        name: 'Heat Furnace', slug: 'heat-furnace', defaultCount: 0, category: 'Environment', hasLevels: false,
        unlocks: { 1: 7 },
        counts: [0, 0, 0, 0, 0, 0, 1],
        tooltip: "Provides Warm or Scorching growing conditions for crops that need one&#10;The calculator picks whichever mode is more profitable&#10;Covers a 9x9 area around itself; how many plots fit depends on what shares it"
    },
    {
        name: 'Cooling Unit', slug: 'cooling-unit', defaultCount: 0, category: 'Environment', hasLevels: false,
        unlocks: { 1: 7 },
        counts: [0, 0, 0, 0, 0, 0, 1],
        tooltip: "Provides Cool or Freeze growing conditions for crops that need one&#10;The calculator picks whichever mode is more profitable&#10;Covers a 9x9 area around itself; how many plots fit depends on what shares it"
    },
    {
        name: 'Sunlamp', slug: 'sunlamp', defaultCount: 0, category: 'Environment', hasLevels: false,
        unlocks: { 1: 9 },
        counts: [0, 0, 0, 0, 0, 0, 0, 0, 1],
        tooltip: "Provides Adequate growing conditions for crops that need one&#10;Covers a 9x9 area around itself; how many plots fit depends on what shares it"
    },
    {
        name: 'Carousel Mill', slug: 'carousel-mill', defaultCount: 1, category: 'Materials Processing', hasWorker: true, ability: 'Wind', personality: 'Tenacious',
        unlocks: { 1: 2, 2: 5, 3: 9, 4: 13, 5: 16, 6: 18 },
        counts: [0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        tooltip: "Lv.1: wheatmeal&#10;Lv.2: tofu, milled rice&#10;Lv.3: lavender powder&#10;Lv.4: rice drink, ginseng powder&#10;Lv.5: refined flour, coconut oil&#10;Lv.6: cocoa powder, coconut milk"
    },
    {
        name: 'Crafting Table', slug: 'crafting-table', defaultCount: 1, category: 'Materials Processing', hasWorker: true, ability: 'Artisanship', personality: 'Judicious',
        unlocks: { 1: 3, 2: 5, 3: 7, 4: 9, 5: 12, 6: 15, 7: 18, 8: 20 },
        counts: [0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        tooltip: "Lv.1: wood sculpture&#10;Lv.2: bamboo ware, river-washed stones, premium river-washed stones&#10;Lv.3: rose freshener, pottery, premium rose freshener&#10;Lv.4: bouquet, shell ornament, lavender sachet&#10;Lv.5: wind chime, star wish lantern, dream catcher, advanced wind chime&#10;Lv.6: rubber duck, pearl necklace, woven toy, porcelain&#10;Lv.7: dye, gemstone dust, flowers in a bottle, advanced gemstone dust&#10;Lv.8: doll&#10;Some recipes need ingredients from facilities not yet in the calculator"
    },
    {
        name: 'Claw Game Cooker', slug: 'claw-game-cooker', defaultCount: 1, category: 'Materials Processing', hasWorker: true, ability: 'Fire', personality: 'Practical',
        unlocks: { 1: 4, 2: 5, 3: 7, 4: 9, 5: 12, 6: 16, 7: 19 },
        counts: [0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        tooltip: "Lv.1: bread, premium bread&#10;Lv.2: roasted soybeans&#10;Lv.3: maple candy roasted potatoes, apple tart, rose shortbread&#10;Lv.4: lavender cookies, apple candy&#10;Lv.5: grape candy, caramel nut chips&#10;Lv.6: maple candy star, coconut cookie&#10;Lv.7: flower bread, berry chocolate coconut pudding, premium berry chocolate coconut pudding&#10;Some recipes need ingredients from facilities not yet in the calculator"
    },
    {
        name: 'Jukebox Dryer', slug: 'jukebox-dryer', defaultCount: 1, category: 'Materials Processing', hasWorker: true, ability: 'Dark', personality: 'Nimble',
        unlocks: { 1: 4, 2: 5, 3: 7, 4: 10, 5: 12, 6: 14, 7: 18 },
        counts: [0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        tooltip: "Lv.1: potato chips&#10;Lv.2: dried lemon slices&#10;Lv.3: dried cherry blossom, dried bean curd&#10;Lv.4: dried apple slices, dried strawberries&#10;Lv.5: nuts, dried ginseng&#10;Lv.6: dried grapes, shredded coconut&#10;Lv.7: dried cranberries, dried flowers"
    },
    {
        name: 'Simmering Pot', slug: 'simmering-pot', defaultCount: 0, category: 'Materials Processing', hasWorker: true, ability: 'Fire', personality: 'Tenacious',
        unlocks: { 1: 5, 2: 7, 3: 9, 4: 12, 5: 15, 6: 18 },
        counts: [0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        tooltip: "Lv.1: plain rice porridge&#10;Lv.2: rose concentrate&#10;Lv.3: rock candy, strawberry jam, maple candy apple jam&#10;Lv.4: chestnut puree, grape jam, ginseng porridge&#10;Lv.5: maple sugar chunk, malt sugar&#10;Lv.6: cocoa spread, cranberry jam, agave syrup"
    },
    {
        name: 'Phonolfactory Table', slug: 'phonolfactory-table', defaultCount: 0, category: 'Materials Processing', hasWorker: true, ability: 'Perfumery', personality: 'Instinctive',
        unlocks: { 1: 6, 2: 7, 3: 10, 4: 14, 5: 17, 6: 19 },
        counts: [0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        tooltip: "Lv.1: bamboo joss stick&#10;Lv.2: rose incense, cherry incense&#10;Lv.3: lavender incense, lemon incense, advanced lemon incense&#10;Lv.4: herbal ginseng aroma&#10;Lv.5: soap, premium soap&#10;Lv.6: orange flower incense, mixed perfume, lotion, premium mixed perfume"
    },
    {
        name: 'Bouncy Brew Keg', slug: 'bouncy-brew-keg', defaultCount: 0, category: 'Materials Processing', hasWorker: true, ability: 'Water', personality: 'Energetic',
        unlocks: { 1: 6, 2: 9, 3: 13, 4: 17, 5: 19 },
        counts: [0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        tooltip: "Lv.1: wheat tea, toasted rice green tea&#10;Lv.2: potato kvass, strawberry juice, apple juice, sugarcane juice&#10;Lv.3: grape juice, ginseng water, grape lemon drink, walnut milk&#10;Lv.4: cranberry juice, coconut cooler&#10;Lv.5: agave drink, hot cocoa, coconut cocoa, orange flower dew"
    },
    {
        name: 'Blazing Stove', slug: 'blazing-stove', defaultCount: 0, category: 'Materials Processing', hasWorker: true, ability: 'Fire', personality: 'Nimble',
        unlocks: { 1: 8, 2: 10, 3: 13, 4: 16, 5: 18 },
        counts: [0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        tooltip: "Lv.1: soy sauce fried rice, creamy potato soup, cherry blossom rice ball, premium potato soup&#10;Lv.2: tanghulu, soy sauce tofu, sugar-roasted chestnuts&#10;Lv.3: steamed vermicelli roll, ginseng chestnut cake, walnut cake&#10;Lv.4: jello, strawberry candy, rich grape compote, premium jello&#10;Lv.5: strawberry cream puff, cranberry chocolate&#10;Not yet verified in game."
    },
    {
        name: 'Pickling Jar', slug: 'pickling-jar', defaultCount: 0, category: 'Materials Processing', hasWorker: true, ability: 'Dark', personality: 'Playful',
        unlocks: { 1: 8, 2: 10, 3: 13, 4: 16, 5: 19 },
        counts: [0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        tooltip: "Lv.1: soy sauce, salted cherry blossom&#10;Lv.2: sweet rice drink, cider vinegar, premium sweet rice wine&#10;Lv.3: rice vinegar, salted lemon, premium salted lemon&#10;Lv.4: candied strawberries&#10;Lv.5: candied orange flower&#10;Not yet verified in game."
    },
    {
        name: 'Joy Wheel Loom', slug: 'joy-wheel-loom', defaultCount: 0, category: 'Materials Processing', hasWorker: true, ability: 'Wind', personality: 'Faithful',
        unlocks: { 1: 7, 2: 10, 3: 15, 4: 19 },
        counts: [0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        tooltip: "Lv.1: cotton thread&#10;Lv.2: woolen yarn, cotton fabric&#10;Lv.3: palm rope, wool fabric&#10;Lv.4: dyed cotton fabric&#10;Not yet verified in game."
    },
    {
        name: 'Woodworking Bench', slug: 'woodworking-bench', defaultCount: 0, category: 'Materials Processing', hasWorker: true, ability: 'Artisanship', personality: 'Energetic',
        unlocks: { 1: 6, 2: 10, 3: 14, 4: 18 },
        counts: [0, 0, 0, 0, 0, 1],
        tooltip: "Lv.1: rough lumber&#10;Lv.2: standard planks&#10;Lv.3: laminated beams&#10;Lv.4: densified timber component&#10;Turns Wood Blocks into RV level-up materials."
    },
    {
        name: 'Chimney Kiln', slug: 'chimney-kiln', defaultCount: 0, category: 'Materials Processing', hasWorker: true, ability: 'Fire', personality: 'Practical',
        unlocks: { 1: 6, 2: 10, 3: 14, 4: 18 },
        counts: [0, 0, 0, 0, 0, 1],
        tooltip: "Lv.1: coarse-sifted ore&#10;Lv.2: sintered ore brick&#10;Lv.3: refined ore&#10;Lv.4: microcrystalline ore plate&#10;Turns Mineral Sand into RV level-up materials."
    },
];

// What reaching each RV level costs: coins, plus raw Wood Blocks and Mineral Sand up to RV 6 and
// one Woodworking Bench item and one Chimney Kiln item from RV 7.
export const LEVEL_UP_COSTS = {
    2: { coins: 140, items: [['wood_block', 3]] },
    3: { coins: 800, items: [['wood_block', 25]] },
    4: { coins: 2900, items: [['wood_block', 100], ['mineral_sand', 120]] },
    5: { coins: 7300, items: [['wood_block', 550], ['mineral_sand', 250]] },
    6: { coins: 32000, items: [['wood_block', 1300], ['mineral_sand', 700]] },
    7: { coins: 69000, items: [['rough_lumber', 290], ['coarse_sifted_ore', 360]] },
    8: { coins: 180000, items: [['rough_lumber', 1100], ['coarse_sifted_ore', 640]] },
    9: { coins: 260000, items: [['rough_lumber', 1520], ['coarse_sifted_ore', 800]] },
    10: { coins: 510000, items: [['rough_lumber', 2000], ['coarse_sifted_ore', 2400]] },
    11: { coins: 680000, items: [['standard_planks', 320], ['sintered_ore_brick', 350]] },
    12: { coins: 1060000, items: [['standard_planks', 910], ['sintered_ore_brick', 480]] },
    13: { coins: 1930000, items: [['standard_planks', 1230], ['sintered_ore_brick', 760]] },
    14: { coins: 2620000, items: [['standard_planks', 1590], ['sintered_ore_brick', 1060]] },
    15: { coins: 3760000, items: [['laminated_beams', 390], ['refined_ore', 150]] },
    16: { coins: 4900000, items: [['laminated_beams', 480], ['refined_ore', 310]] },
    17: { coins: 8630000, items: [['laminated_beams', 630], ['refined_ore', 380]] },
    18: { coins: 11600000, items: [['laminated_beams', 800], ['refined_ore', 520]] },
    19: { coins: 17100000, items: [['densified_timber_component', 400], ['microcrystalline_ore_plate', 220]] },
    20: { coins: 20800000, items: [['densified_timber_component', 490], ['microcrystalline_ore_plate', 270]] },
};

// The Woodworking Bench and Chimney Kiln chains, lowest tier first: what a player might have in
// stock toward a level-up.
export const LEVEL_UP_CHAINS = [
    ['wood_block', 'rough_lumber', 'standard_planks', 'laminated_beams', 'densified_timber_component'],
    ['mineral_sand', 'coarse_sifted_ore', 'sintered_ore_brick', 'refined_ore', 'microcrystalline_ore_plate'],
];

// Display order for facility categories. Auxiliary facilities (Storage Unit, power/climate
// buildings) are deliberately excluded here: they don't produce items.
export const FACILITY_CATEGORIES = ['Materials', 'Environment', 'Aniimo Materials', 'Materials Processing'];

// Facility name -> category, so other pages can group by the same categories as the facility
// input cards (Materials/Aniimo Materials are grower facilities, Materials Processing is processor
// facilities).
export const FACILITY_CATEGORY_BY_NAME = new Map(FACILITIES.map(f => [f.name, f.category]));

// Highest RV (Homeland) level in the game.
export const MAX_HOME_LEVEL = 20;

// Highest level of each upgrade module at each RV level (index = RV level - 1).
export const MODULE_MAX_LEVELS = {
    ecological_module: [0, 0, 1, 1, 1, 1, 2, 3, 3, 3, 4, 5, 5, 6, 6, 6, 7, 8, 8, 8],
    kitchen_module: [0, 1, 1, 2, 2, 2, 2, 3, 3, 4, 4, 4, 5, 5, 5, 6, 6, 6, 7, 7],
    resource_detector: [0, 0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 4, 5, 5, 6, 6, 7, 7, 8, 8],
    crafting_module: [0, 0, 0, 0, 1, 1, 2, 2, 2, 3, 3, 4, 4, 4, 4, 4, 5, 6, 7, 7],
};

// The value for RV level `homeLevel` in a per-RV list, keeping the last value past its end.
function atHomeLevel(list, homeLevel) {
    return list[Math.min(homeLevel, list.length) - 1];
}

// How many Aniimo can live on the homeland at each RV level (index = RV level - 1; `null` where
// unknown).
export const ANIIMO_MAX = [null, 8, 11, 14, 17, 20, 22, 24, 26, 28, 30, 32, 34, 36, 38, 40, 42, 43, 44, 45];

// Highest RV level whose building counts have been confirmed in game.
export const COUNTS_CONFIRMED_UP_TO = 8;

// Everything a player at `homeLevel` could have: each facility at its highest unlocked level, as
// many as that RV level allows (see `counts`), and every module at its cap for that RV level. Returns the same shapes simple
// mode sends to the solver: `{ facilities: { name: [{count, level}] }, modules }`.
export function simpleSetup(homeLevel) {
    const facilities = {};
    FACILITIES.forEach(f => {
        const unlocked = Object.entries(f.unlocks || {})
            .filter(([, need]) => need <= homeLevel)
            .map(([level]) => Number(level));
        if (unlocked.length === 0) {
            facilities[f.name] = [{ count: 0, level: 1 }];
            return;
        }
        facilities[f.name] = [{ count: atHomeLevel(f.counts, homeLevel), level: Math.max(...unlocked) }];
    });
    const modules = Object.fromEntries(
        Object.entries(MODULE_MAX_LEVELS).map(([module, caps]) => [module, atHomeLevel(caps, homeLevel)])
    );
    return { facilities, modules };
}

