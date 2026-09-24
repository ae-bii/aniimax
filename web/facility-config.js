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
// the last count. Simple mode uses both (see `simpleSetup`). Counts are confirmed in game up to RV
// level 11; past that, Farmland, Woodland and Mine follow the game's pattern and the rest keep
// their RV 11 count.
//
// Facilities marked "Not yet verified in game" in their tooltip haven't had their numbers
// confirmed in game yet.
export const FACILITIES = [
    {
        name: 'Farmland', slug: 'farmland', defaultCount: 1, category: 'Materials',
        unlocks: { 1: 1, 2: 2, 3: 5, 4: 7, 5: 9, 6: 12, 7: 16 },
        counts: [4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30, 32, 34, 36, 38, 40, 42],
        tooltip: "Lv.1: Wheat&#10;Lv.2: Potato, Quick Wheat&#10;Lv.3: Rice, Soybean&#10;Lv.4: Rose, Cotton, Quick Potato&#10;Lv.5: Strawberry, Lavender, Sugarcane&#10;Lv.6: Ginseng, Grape, Premium Wheat, Quick Rice&#10;Lv.7: Cranberry, Agave, Quick Strawberry"
    },
    {
        name: 'Woodland', slug: 'woodland', defaultCount: 1, category: 'Materials',
        unlocks: { 1: 2, 2: 4, 3: 7, 4: 11, 5: 14, 6: 18 },
        counts: [0, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21],
        tooltip: "Lv.1: Willow Wood&#10;Lv.2: Bamboo, Lemon&#10;Lv.3: Cherry Blossom, Apple, Maple Syrup, Quick Bamboo&#10;Lv.4: Palm Bark, Chestnut, Walnut, Quick Lemon&#10;Lv.5: Natural Rubber, Coconut, Quick Maple Syrup&#10;Lv.6: Cocoa, Orange Flower, Quick Coconut&#10;Also yields Wood Blocks."
    },
    {
        name: 'Mine', slug: 'mine', defaultCount: 1, category: 'Materials', hasWorker: true, ability: 'Earth', personality: 'Playful',
        unlocks: { 1: 3, 2: 6, 3: 9, 4: 12, 5: 15, 6: 18 },
        counts: [0, 0, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10],
        tooltip: "Lv.1: Rock&#10;Lv.2: Clay&#10;Lv.3: Shell&#10;Lv.4: Copper Ore&#10;Lv.5: Quartz Ore&#10;Lv.6: Gem&#10;Also yields Mineral Sand."
    },
    {
        name: 'Well', slug: 'well', defaultCount: 0, category: 'Materials', hasWorker: true, ability: 'Water', personality: 'Faithful',
        unlocks: { 1: 4, 2: 8, 3: 11, 4: 13, 5: 17 },
        counts: [0, 0, 0, 1, 1, 1, 1, 2],
        tooltip: "Lv.1: Well Water, Quick Well Water&#10;Lv.2: Fresh Water&#10;Lv.3: Quick Fresh Water&#10;Lv.4: Deep Rock Spring Water, Quick Deep Rock Spring Water&#10;Lv.5: Natural Mineral Spring Water, Quick Natural Mineral Spring Water"
    },
    {
        name: 'Tidewhisper Sandcastle', slug: 'tidewhisper-sandcastle', defaultCount: 0, category: 'Aniimo Materials', hasWorker: true, ability: 'Leisure', personality: 'Judicious',
        unlocks: { 1: 5, 2: 8, 3: 13 },
        counts: [0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        tooltip: "Lv.1: Sea Salt&#10;Lv.2: Quick Sea Salt&#10;Lv.3: Pearl (needs Warm)"
    },
    {
        name: 'Dewy House', slug: 'dewy-house', defaultCount: 0, category: 'Aniimo Materials', hasWorker: true, ability: 'Leisure', personality: 'Instinctive',
        unlocks: { 1: 6, 2: 11 },
        counts: [0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        tooltip: "Lv.1: Aromathyst&#10;Lv.2: Quick Aromathyst"
    },
    {
        name: 'Nimbus Bed', slug: 'nimbus-bed', defaultCount: 0, category: 'Aniimo Materials', hasWorker: true, ability: 'Leisure', personality: 'Judicious',
        unlocks: { 1: 10, 2: 13, 3: 16 },
        counts: [0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        tooltip: "Lv.1: Wool&#10;Lv.2: Quick Wool&#10;Lv.3: Petals"
    },
    {
        name: 'Starfall Hammock', slug: 'starfall-hammock', defaultCount: 0, category: 'Aniimo Materials', hasLevels: false, hasWorker: true, ability: 'Leisure', personality: 'Faithful',
        unlocks: { 1: 12 },
        counts: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        tooltip: "Star (needs Cool)&#10;Not yet verified in game."
    },
    {
        name: 'Floral Windmill', slug: 'floral-windmill', defaultCount: 0, category: 'Aniimo Materials', hasLevels: false, hasWorker: true, ability: 'Leisure', personality: 'Nimble',
        unlocks: { 1: 18 },
        counts: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1],
        tooltip: "Scales, Quick Scales (need Adequate)&#10;Not yet verified in game."
    },
    {
        name: 'Heat Furnace', slug: 'heat-furnace', defaultCount: 0, category: 'Environment', hasLevels: false,
        unlocks: { 1: 7 },
        counts: [0, 0, 0, 0, 0, 0, 1],
        tooltip: "Provides Warm or Scorching growing conditions for crops that need one&#10;The calculator picks whichever mode is more profitable.&#10;Covers a 9x9 area around itself; how many plots fit depends on what shares it."
    },
    {
        name: 'Cooling Unit', slug: 'cooling-unit', defaultCount: 0, category: 'Environment', hasLevels: false,
        unlocks: { 1: 7 },
        counts: [0, 0, 0, 0, 0, 0, 1],
        tooltip: "Provides Cool or Freeze growing conditions for crops that need one&#10;The calculator picks whichever mode is more profitable.&#10;Covers a 9x9 area around itself; how many plots fit depends on what shares it."
    },
    {
        name: 'Sunlamp', slug: 'sunlamp', defaultCount: 0, category: 'Environment', hasLevels: false,
        unlocks: { 1: 9 },
        counts: [0, 0, 0, 0, 0, 0, 0, 0, 1],
        tooltip: "Provides Adequate growing conditions for crops that need one&#10;Covers a 9x9 area around itself; how many plots fit depends on what shares it."
    },
    {
        name: 'Carousel Mill', slug: 'carousel-mill', defaultCount: 1, category: 'Materials Processing', hasWorker: true, ability: 'Wind', personality: 'Tenacious',
        unlocks: { 1: 2, 2: 5, 3: 9, 4: 13, 5: 16, 6: 18 },
        counts: [0, 1, 1, 1, 1, 1, 1, 1, 2],
        tooltip: "Lv.1: Wheatmeal&#10;Lv.2: Tofu, Milled Rice&#10;Lv.3: Lavender Powder&#10;Lv.4: Rice Drink, Ginseng Powder&#10;Lv.5: Refined Flour, Coconut Oil&#10;Lv.6: Cocoa Powder, Coconut Milk"
    },
    {
        name: 'Crafting Table', slug: 'crafting-table', defaultCount: 1, category: 'Materials Processing', hasWorker: true, ability: 'Artisanship', personality: 'Judicious',
        unlocks: { 1: 3, 2: 5, 3: 7, 4: 9, 5: 12, 6: 15, 7: 18, 8: 20 },
        counts: [0, 0, 1, 1, 1, 1, 1, 1, 1, 2],
        tooltip: "Lv.1: Wood Sculpture&#10;Lv.2: Bamboo Ware, River-Washed Stones, Premium River-Washed Stones&#10;Lv.3: Rose Freshener, Pottery, Premium Rose Freshener&#10;Lv.4: Bouquet, Shell Ornament, Lavender Sachet&#10;Lv.5: Wind Chime, Star Wish Lantern, Dream Catcher, Advanced Wind Chime&#10;Lv.6: Rubber Duck, Pearl Necklace, Woven Toy, Porcelain&#10;Lv.7: Dye, Gemstone Dust, Flowers in a Bottle, Advanced Gemstone Dust&#10;Lv.8: Doll"
    },
    {
        name: 'Claw Game Cooker', slug: 'claw-game-cooker', defaultCount: 1, category: 'Materials Processing', hasWorker: true, ability: 'Fire', personality: 'Practical',
        unlocks: { 1: 4, 2: 5, 3: 7, 4: 9, 5: 12, 6: 16, 7: 19 },
        counts: [0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 2],
        tooltip: "Lv.1: Bread, Premium Bread&#10;Lv.2: Roasted Soybeans&#10;Lv.3: Maple Candy Roasted Potatoes, Apple Tart, Rose Shortbread&#10;Lv.4: Lavender Cookies, Apple Candy&#10;Lv.5: Grape Candy, Caramel Nut Chips&#10;Lv.6: Maple Candy Star, Coconut Cookie&#10;Lv.7: Flower Bread, Berry Chocolate Coconut Pudding, Premium Berry Chocolate Coconut Pudding"
    },
    {
        name: 'Jukebox Dryer', slug: 'jukebox-dryer', defaultCount: 1, category: 'Materials Processing', hasWorker: true, ability: 'Dark', personality: 'Nimble',
        unlocks: { 1: 4, 2: 5, 3: 7, 4: 10, 5: 12, 6: 14, 7: 18 },
        counts: [0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 2],
        tooltip: "Lv.1: Potato Chips&#10;Lv.2: Dried Lemon Slices&#10;Lv.3: Dried Cherry Blossom, Dried Bean Curd&#10;Lv.4: Dried Apple Slices, Dried Strawberries&#10;Lv.5: Nuts, Dried Ginseng&#10;Lv.6: Dried Grapes, Shredded Coconut&#10;Lv.7: Dried Cranberries, Dried Flowers"
    },
    {
        name: 'Simmering Pot', slug: 'simmering-pot', defaultCount: 0, category: 'Materials Processing', hasWorker: true, ability: 'Fire', personality: 'Tenacious',
        unlocks: { 1: 5, 2: 7, 3: 9, 4: 12, 5: 15, 6: 18 },
        counts: [0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        tooltip: "Lv.1: Plain Rice Porridge&#10;Lv.2: Rose Concentrate&#10;Lv.3: Rock Candy, Strawberry Jam, Maple Candy Apple Jam&#10;Lv.4: Chestnut Puree, Grape Jam, Ginseng Porridge&#10;Lv.5: Maple Sugar Chunk, Malt Sugar&#10;Lv.6: Cocoa Spread, Cranberry Jam, Agave Syrup"
    },
    {
        name: 'Phonolfactory Table', slug: 'phonolfactory-table', defaultCount: 0, category: 'Materials Processing', hasWorker: true, ability: 'Perfumery', personality: 'Instinctive',
        unlocks: { 1: 6, 2: 7, 3: 10, 4: 14, 5: 17, 6: 19 },
        counts: [0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        tooltip: "Lv.1: Bamboo Joss Stick&#10;Lv.2: Rose Incense, Cherry Incense&#10;Lv.3: Lavender Incense, Lemon Incense, Advanced Lemon Incense&#10;Lv.4: Herbal Ginseng Aroma&#10;Lv.5: Soap, Premium Soap&#10;Lv.6: Orange Flower Incense, Mixed Perfume, Lotion, Premium Mixed Perfume"
    },
    {
        name: 'Bouncy Brew Keg', slug: 'bouncy-brew-keg', defaultCount: 0, category: 'Materials Processing', hasWorker: true, ability: 'Water', personality: 'Energetic',
        unlocks: { 1: 6, 2: 9, 3: 13, 4: 17, 5: 19 },
        counts: [0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        tooltip: "Lv.1: Wheat Tea, Toasted Rice Green Tea&#10;Lv.2: Potato Kvass, Strawberry Juice, Apple Juice, Sugarcane Juice&#10;Lv.3: Grape Juice, Ginseng Water, Grape Lemon Drink, Walnut Milk&#10;Lv.4: Cranberry Juice, Coconut Cooler&#10;Lv.5: Agave Drink, Hot Cocoa, Coconut Cocoa, Orange Flower Dew"
    },
    {
        name: 'Blazing Stove', slug: 'blazing-stove', defaultCount: 0, category: 'Materials Processing', hasWorker: true, ability: 'Fire', personality: 'Nimble',
        unlocks: { 1: 8, 2: 10, 3: 13, 4: 16, 5: 18 },
        counts: [0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        tooltip: "Lv.1: Soy Sauce Fried Rice, Creamy Potato Soup, Cherry Blossom Rice Ball, Premium Potato Soup&#10;Lv.2: Tanghulu, Soy Sauce Tofu, Sugar-Roasted Chestnuts&#10;Lv.3: Steamed Vermicelli Roll, Ginseng Chestnut Cake, Walnut Cake&#10;Lv.4: Jello, Strawberry Candy, Rich Grape Compote, Premium Jello&#10;Lv.5: Strawberry Cream Puff, Cranberry Chocolate"
    },
    {
        name: 'Pickling Jar', slug: 'pickling-jar', defaultCount: 0, category: 'Materials Processing', hasWorker: true, ability: 'Dark', personality: 'Playful',
        unlocks: { 1: 8, 2: 10, 3: 13, 4: 16, 5: 19 },
        counts: [0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        tooltip: "Lv.1: Soy Sauce, Salted Cherry Blossom&#10;Lv.2: Sweet Rice Drink, Cider Vinegar, Premium Sweet Rice Wine&#10;Lv.3: Rice Vinegar, Salted Lemon, Premium Salted Lemon&#10;Lv.4: Candied Strawberries&#10;Lv.5: Candied Orange Flower"
    },
    {
        name: 'Joy Wheel Loom', slug: 'joy-wheel-loom', defaultCount: 0, category: 'Materials Processing', hasWorker: true, ability: 'Wind', personality: 'Faithful',
        unlocks: { 1: 7, 2: 10, 3: 15, 4: 19 },
        counts: [0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        tooltip: "Lv.1: Cotton Thread&#10;Lv.2: Woolen Yarn, Cotton Fabric&#10;Lv.3: Palm Rope, Wool Fabric&#10;Lv.4: Dyed Cotton Fabric"
    },
    {
        name: 'Dance Pad Polisher', slug: 'dance-pad-polisher', defaultCount: 0, category: 'Materials Processing', hasWorker: true, ability: 'Lightning',
        unlocks: { 1: 2, 2: 5, 3: 7 },
        counts: [0, 1],
        tooltip: "Lv.1: Growth Bud&#10;Lv.2: Growth Flower&#10;Lv.3: Growth Fruit&#10;Makes Aniimo EXP, not coins.&#10;Unlock levels not yet confirmed in game."
    },
    {
        name: 'Aniipod Maker', slug: 'aniipod-maker', defaultCount: 0, category: 'Materials Processing', hasWorker: true, ability: 'Lightning',
        unlocks: { 1: 3, 2: 6, 3: 9 },
        counts: [0, 0, 1],
        tooltip: "Lv.1: Aniipod&#10;Lv.2: Aniipod Pro&#10;Lv.3: Aniipod Mega&#10;Aniipods are for catching Aniimo, not for selling.&#10;Unlock levels not yet confirmed in game."
    },
    {
        name: 'Woodworking Bench', slug: 'woodworking-bench', defaultCount: 0, category: 'Materials Processing', hasWorker: true, ability: 'Artisanship', personality: 'Energetic',
        unlocks: { 1: 6, 2: 10, 3: 14, 4: 18 },
        counts: [0, 0, 0, 0, 0, 1, 1, 1, 1, 2],
        tooltip: "Lv.1: Rough Lumber&#10;Lv.2: Standard Planks&#10;Lv.3: Laminated Beams&#10;Lv.4: Densified Timber Component&#10;Turns Wood Blocks into RV level-up materials."
    },
    {
        name: 'Chimney Kiln', slug: 'chimney-kiln', defaultCount: 0, category: 'Materials Processing', hasWorker: true, ability: 'Fire', personality: 'Practical',
        unlocks: { 1: 6, 2: 10, 3: 14, 4: 18 },
        counts: [0, 0, 0, 0, 0, 1, 1, 1, 1, 2],
        tooltip: "Lv.1: Coarse-Sifted Ore&#10;Lv.2: Sintered Ore Brick&#10;Lv.3: Refined Ore&#10;Lv.4: Microcrystalline Ore Plate&#10;Turns Mineral Sand into RV level-up materials."
    },
];

// Aniipod tiers in Aniipod Maker level order: each level adds a better one for catching Aniimo.
// The "Most Aniipods" strategy makes only the best tier the player's Maker can reach.
export const ANIIPOD_TIERS = ['aniipod', 'aniipod_pro', 'aniipod_mega'];

// Recipes unlocked with a rare currency (a few come from RV level-ups or from collecting one of
// every product). Plans leave them out until the player says they have them.
export const SPECIAL_RECIPES = [
    { name: 'rose_shortbread', facility: 'Claw Game Cooker' },
    { name: 'potato_kvass', facility: 'Bouncy Brew Keg' },
    { name: 'ginseng_porridge', facility: 'Simmering Pot' },
    { name: 'strawberry_candy', facility: 'Blazing Stove' },
    { name: 'flowers_in_a_bottle', facility: 'Crafting Table' },
    { name: 'lotion', facility: 'Phonolfactory Table' },
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

