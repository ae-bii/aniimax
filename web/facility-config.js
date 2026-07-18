// Shared facility configuration, used by app.js for both the facility input cards and the
// facility recipe reference modal, so the two stay in sync automatically.

// Facility configuration. `name` must exactly match the facility string used throughout the
// Rust data model (ProductionItem.facility / FacilityCounts keys) since it's sent verbatim as
// the JSON key for each facility's count/level. Add new facilities here only; cards and input
// handling are generated dynamically, no other file needs to change. `category` groups the cards
// in the UI (see `FACILITY_CATEGORIES` below for display order). `hasLevels: false` hides the
// Level input entirely for facilities that don't level up in-game; omit the field (defaults to
// leveled) for any facility that does.
export const FACILITIES = [
    {
        name: 'Farmland', slug: 'farmland', defaultCount: 1, category: 'Materials',
        tooltip: "Lv.1: wheat, quick wheat&#10;Lv.2: potatoes, sugarcane, rice, cotton&#10;Lv.3: strawberries, soybeans&#10;Lv.4: lavender, agave, rose, quick rose&#10;Lv.5: grapes, ginseng, pumpkin"
    },
    {
        name: 'Woodland', slug: 'woodland', defaultCount: 1, category: 'Materials',
        tooltip: "Lv.1: willow&#10;Lv.2: chestnut, bamboo, lemon, quick lemon&#10;Lv.3: palm bark, coconut, maple syrup, quick coconut&#10;Lv.4: natural rubber, walnut, pine tree hardwood&#10;Only source of Wood Blocks"
    },
    {
        name: 'Mineral Pile', slug: 'mineral-pile', defaultCount: 1, category: 'Materials',
        tooltip: "Lv.1: shell, quick shell&#10;Lv.2: clay&#10;Lv.3: quartz, quick quartz&#10;Lv.4: gem&#10;Currently the only confirmed source of Mineral Sand. Times are estimated from workload."
    },
    {
        name: 'Heat Furnace', slug: 'heat-furnace', defaultCount: 0, category: 'Environment', hasLevels: false,
        tooltip: "Provides Warm or Scorching growing conditions for Farmland/Woodland crops that need one&#10;The calculator picks whichever mode is more profitable&#10;One unit covers 24 Farmland, 12 Woodland, or a 12+12 hybrid layout"
    },
    {
        name: 'Cooling Unit', slug: 'cooling-unit', defaultCount: 0, category: 'Environment', hasLevels: false,
        tooltip: "Provides Cool or Freeze growing conditions for Farmland/Woodland crops that need one&#10;The calculator picks whichever mode is more profitable&#10;One unit covers 24 Farmland, 12 Woodland, or a 12+12 hybrid layout"
    },
    {
        name: 'Sunlamp', slug: 'sunlamp', defaultCount: 0, category: 'Environment', hasLevels: false,
        tooltip: "Provides Adequate growing conditions for Farmland/Woodland crops that need one&#10;One unit covers 24 Farmland, 12 Woodland, or a 12+12 hybrid layout"
    },
    {
        name: 'Nimbus Bed', slug: 'nimbus-bed', defaultCount: 0, category: 'Aniimo Materials', hasLevels: false,
        tooltip: "Requires a matching Aniimo Family (Nimbi for Wool, Iris for Petals)&#10;Produces: wool (4), petals (6) per batch&#10;Doesn't level up"
    },
    {
        name: 'Grass Blossom Mat', slug: 'grass-blossom-mat', defaultCount: 0, category: 'Aniimo Materials', hasLevels: false,
        tooltip: "scales, quick scales&#10;Doesn't level up. Sell values/times are estimates"
    },
    {
        name: 'Starfall Hammock', slug: 'starfall-hammock', defaultCount: 0, category: 'Aniimo Materials', hasLevels: false,
        tooltip: "star (needs Cool environment)&#10;Doesn't level up. Environment requirement not enforced yet"
    },
    {
        name: 'Tidewhisper Sandcastle', slug: 'tidewhisper-sandcastle', defaultCount: 0, category: 'Aniimo Materials', hasLevels: false,
        tooltip: "pearl (needs Cool environment), love bubble (needs Freeze environment)&#10;Doesn't level up. Environment requirement not enforced yet"
    },
    {
        name: 'Dewy House', slug: 'dewy-house', defaultCount: 0, category: 'Aniimo Materials', hasLevels: false,
        tooltip: "aromathyst (needs Warm environment)&#10;Doesn't level up. Environment requirement not enforced yet"
    },
    {
        name: 'Carousel Mill', slug: 'carousel-mill', defaultCount: 1, category: 'Materials Processing',
        tooltip: "Lv.1: wheatmeal&#10;Lv.2: rice processed&#10;Lv.3: tofu, coconut oil"
    },
    {
        name: 'Phonolfactory Table', slug: 'phonolfactory-table', defaultCount: 1, category: 'Materials Processing',
        tooltip: "Lv.1: lemon incense&#10;Lv.2: lavender incense, rose incense&#10;Lv.3: deluxe lavender incense, premium rose/lemon incense, soap, rose freshener, cedarwood incense&#10;Lv.4: sachet, lotion, deluxe cedarwood incense, mixed perfume, coconutty candle&#10;Higher-tier recipes need Cotton Fabric, not yet available from any facility"
    },
    {
        name: 'Bouncy Brew Keg', slug: 'bouncy-brew-keg', defaultCount: 1, category: 'Materials Processing',
        tooltip: "Lv.1: soy sauce, sweet rice drink, strawberry jam, wheat tea&#10;Lv.2: tequila highball, fermented rice drink, potato kvass, advanced potato kvass&#10;Lv.3: grape juice, tequila soda&#10;Some recipes need Rock Candy/Grape Candy from Claw Game Cooker"
    },
    {
        name: 'Crafting Table', slug: 'crafting-table', defaultCount: 1, category: 'Materials Processing',
        tooltip: "Lv.1: shell ornament, wood sculpture, premium wood sculpture&#10;Lv.2: bamboo ware, wind chime, pottery, advanced wind chime&#10;Lv.3: pearl necklace, bubble bracelet, porcelain, woven toy, bracelet, dye, premium woven toy&#10;Lv.4: dream catcher, gemstone dust, advanced dream catcher&#10;Lv.5: flowers in a bottle, bouquet, starwish lantern, doll&#10;Premium/Advanced items sell for Bud Tickets"
    },
    {
        name: 'Claw Game Cooker', slug: 'claw-game-cooker', defaultCount: 1, category: 'Materials Processing',
        tooltip: "Lv.1: malt sugar, rock candy, sugar-roasted chestnuts, flower bread, advanced flower bread&#10;Lv.2: strawberry candy, tanghulu, maple candy(+star), strawberry cream puff, tofu cake, soy sauce tofu(+advanced), soy sauce fried rice, creamy potato bisque&#10;Lv.3: coconut cookie, grape candy, jello, pumpkin rice cake&#10;Some recipes need Star, not yet available from any facility"
    },
    {
        name: 'Joy Wheel Loom', slug: 'joy-wheel-loom', defaultCount: 0, category: 'Materials Processing',
        tooltip: "Lv.1: cotton thread, woolen yarn&#10;Lv.2: palm rope&#10;Lv.3: cotton fabric, wool fabric&#10;Lv.4: palm fabric"
    },
    {
        name: 'Jukebox Dryer', slug: 'jukebox-dryer', defaultCount: 1, category: 'Materials Processing',
        tooltip: "Lv.1: potato chips&#10;Lv.2: dried strawberries, dried lemon slices&#10;Lv.3: dried bean curd, dried flowers, shredded coconut&#10;Lv.4: nuts, herbs, dried grapes, caramel nut chips"
    },
];

// Display order for facility categories. Auxiliary facilities (Storage Unit, power/climate
// buildings) are deliberately excluded here: they don't produce items.
export const FACILITY_CATEGORIES = ['Materials', 'Environment', 'Aniimo Materials', 'Materials Processing'];

// Facility name -> category, so other pages can group by the same categories as the facility
// input cards (Materials/Aniimo Materials are grower facilities, Materials Processing is
// processor facilities).
export const FACILITY_CATEGORY_BY_NAME = new Map(FACILITIES.map(f => [f.name, f.category]));
