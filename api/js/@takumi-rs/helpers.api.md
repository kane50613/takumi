# @takumi-rs/helpers

## `@takumi-rs/helpers`

```ts
import { CSSProperties } from 'react';

export type AnimationRule = {
    keyframes: string;
    steps: {
        offset: string;
        style?: Declarations;
    }[];
};

// Warning: (ae-forgotten-export) The symbol "WeightRange" needs to be exported
type AxisValue = number | WeightRange;

export type CodepointSource = string | Node_2 | (string | Node_2)[];

// @deprecated
export function collectCodepoints(source: CodepointSource): Set<number>;

export function container(props: Omit<ContainerNode, "type">): ContainerNode;

export type ContainerNode = NodeMetadata & {
    type: "container";
    children?: Node_2[];
};

export type CssInput = string | StyleRule | AnimationRule | MediaRule | SupportsRule | LayerRule;

export type Declarations = CSSProperties;

export const defaultMaxFetchBytes: number;

export function em(em: number): `${number}em`;

export interface FetchedImage {
    data: ArrayBuffer;
    src: string;
}

export type FetchLike = (input: string, init?: RequestInit) => Promise<Response>;

export function fetchOk(url: string, options?: FetchOptions & {
    init?: RequestInit;
}): Promise<Response>;

export type FetchOptions = {
    fetch?: FetchLike;
    timeout?: number;
    signal?: AbortSignal;
    maxBytes?: number;
    allowUrl?: (url: string) => boolean;
};

export function fontFromUrl(url: string, options?: FetchOptions): {
    key: string;
    data: () => Promise<ArrayBuffer>;
};

type FontStyle = "normal" | "italic";

export type FontSubset = {
    name: string;
    key: string;
    weight?: number;
    style?: string;
    unicodeRange: [number, number][];
    generic?: GenericFontFamily;
    data: () => Promise<ArrayBuffer>;
};

export function fr(fr: number): `${number}fr`;

export type GenericFontFamily = "serif" | "sans-serif" | "monospace" | "cursive" | "fantasy" | "system-ui" | "ui-serif" | "ui-sans-serif" | "ui-monospace" | "ui-rounded" | "emoji" | "math" | "fangsong";

// Warning: (ae-forgotten-export) The symbol "GoogleFontName" needs to be exported
// Warning: (ae-forgotten-export) The symbol "KnownGoogleFontFamily" needs to be exported
export type GoogleFontFamily = GoogleFontName | (string & {}) | KnownGoogleFontFamily | {
    name: GoogleFontName | (string & {});
    weight?: number | number[] | WeightRange;
    style?: FontStyle | FontStyle[];
    axes?: Record<string, AxisValue>;
    generic?: GenericFontFamily;
};

// Warning: (ae-forgotten-export) The symbol "GoogleFontShapeFamilies" needs to be exported
type GoogleFontName = GoogleFontShapeFamilies[keyof GoogleFontShapeFamilies];

export function googleFonts(options: GoogleFontsOptions | GoogleFontFamily[]): Promise<FontSubset[]>;

interface GoogleFontShapeFamilies {
    S0: "ABeeZee" | "Alumni Sans Collegiate One" | "Alumni Sans Inline One" | "Alumni Sans Pinstripe" | "Arapey" | "Bevan" | "Changa One" | "Chau Philomene One" | "Crete Round" | "DM Serif Display" | "DM Serif Text" | "Fanwood Text" | "Fondamento" | "Fragment Mono" | "Fruktur" | "Geo" | "IM Fell DW Pica" | "IM Fell Double Pica" | "IM Fell English" | "IM Fell French Canon" | "IM Fell Great Primer" | "Instrument Serif" | "Linden Hill" | "Mate" | "Oregano" | "PT Serif Caption" | "Playwrite AT Guides" | "Playwrite GB J Guides" | "Playwrite GB S Guides" | "Poly" | "Radley" | "Ropa Sans" | "Rosarivo" | "Sanchez" | "Sedan" | "Sorts Mill Goudy" | "Tiro Bangla" | "Tiro Devanagari Hindi" | "Tiro Devanagari Marathi" | "Tiro Devanagari Sanskrit" | "Tiro Gurmukhi" | "Tiro Kannada" | "Tiro Tamil" | "Tiro Telugu" | "Xanh Mono" | "Zen Loop";
    S1: "ADLaM Display" | "Abel" | "Aboreto" | "Abril Fatface" | "Abyssinica SIL" | "Aclonica" | "Acme" | "Actor" | "Adamina" | "Agbalumo" | "Aguafina Script" | "Akaya Kanadaka" | "Akaya Telivigala" | "Akronim" | "Aladin" | "Alata" | "Alatsi" | "Aldrich" | "Alex Brush" | "Alfa Slab One" | "Alice" | "Alien Block" | "Alike" | "Alike Angular" | "Alkalami" | "Allerta" | "Allerta Stencil" | "Allison" | "Allkin" | "Allura" | "Almendra Display" | "Almendra SC" | "Amarante" | "Amethysta" | "Amiri Quran" | "Angkor" | "Annie Use Your Telescope" | "Anta" | "Antic" | "Antic Didone" | "Antic Slab" | "Anton" | "Anton SC" | "Aoboshi One" | "Arbutus" | "Arbutus Slab" | "Architects Daughter" | "Archivo Black" | "Are You Serious" | "Arizonia" | "Armata" | "Artifika" | "Asar" | "Asimovian" | "Asset" | "Atomic Age" | "Aubrey" | "Audiowide" | "Autour One" | "Average" | "Average Sans" | "Averia Gruesa Libre" | "BBH Bartle" | "BBH Bogle" | "BBH Hegarty" | "Babylonica" | "Bacasime Antique" | "Bad Script" | "Badeen Display" | "Bagel Fat One" | "Bahiana" | "Bahianita" | "Bakbak One" | "Balthazar" | "Bangers" | "Barriecito" | "Barrio" | "Basic" | "Baumans" | "Bayon" | "Beau Rivage" | "Bebas Neue" | "Belgrano" | "Bellefair" | "Belleza" | "Benne" | "Bentham" | "Berkshire Swash" | "Betania Patmos" | "Betania Patmos GDL" | "Betania Patmos In" | "Betania Patmos In GDL" | "Beth Ellen" | "BhuTuka Expanded One" | "Bigelow Rules" | "Bigshot One" | "Bilbo" | "Bilbo Swash Caps" | "Birthstone" | "Black And White Picture" | "Black Han Sans" | "Black Ops One" | "Blaka" | "Blaka Hollow" | "Blaka Ink" | "Bokor" | "Boldonse" | "Bonbon" | "Bonheur Royale" | "Boogaloo" | "Borel" | "Bowlby One" | "Bowlby One SC" | "Bpmf Huninn" | "Bpmf Iansui" | "Bpmf Zihi Kai Std" | "Braah One" | "Bree Serif" | "Bruno Ace" | "Bruno Ace SC" | "Bubblegum Sans" | "Bubbler One" | "Bungee" | "Bungee Hairline" | "Bungee Inline" | "Bungee Outline" | "Bungee Shade" | "Bungee Spice" | "Bungee Tint" | "Butcherman" | "Butterfly Kids" | "Bytesized" | "Caacupe One" | "Cactus Classical Serif" | "Caesar Dressing" | "Cagliostro" | "Cal Sans" | "Calistoga" | "Calligraffitti" | "Cambo" | "Candal" | "Cantata One" | "Cantora One" | "Caprasimo" | "Capriola" | "Caramel" | "Carattere" | "Carme" | "Carrois Gothic" | "Carrois Gothic SC" | "Carter One" | "Castoro Titling" | "Caveat Brush" | "Cedarville Cursive" | "Ceviche One" | "Chango" | "Chela One" | "Chelsea Market" | "Chenla" | "Cherish" | "Cherry Bomb One" | "Cherry Cream Soda" | "Chewy" | "Chicle" | "Chilanka" | "Chocolate Classical Sans" | "Chokokutai" | "Chonburi" | "Clicker Script" | "Coiny" | "Combo" | "Comforter" | "Comforter Brush" | "Coming Soon" | "Concert One" | "Condiment" | "Contrail One" | "Convergence" | "Cookie" | "Copse" | "Coral Pixels" | "Courgette" | "Covered By Your Grace" | "Crafty Girls" | "Creepster" | "Croissant One" | "Crushed" | "Cute Font" | "Cutive" | "Cutive Mono" | "Damion" | "Dangrek" | "Darumadrop One" | "Dawning of a New Day" | "Days One" | "Dekko" | "Dela Gothic One" | "Delicious Handrawn" | "Delius" | "Delius Swash Caps" | "Della Respira" | "Denk One" | "Devonshire" | "Dhurjati" | "Didact Gothic" | "Diphylleia" | "Diplomata" | "Diplomata SC" | "Do Hyeon" | "Dokdo" | "Donegal One" | "Doppio One" | "Dorsa" | "DotGothic16" | "Dr Sugiyama" | "Duru Sans" | "Dynalight" | "Eagle Lake" | "East Sea Dokdo" | "Eater" | "Electrolize" | "Emblema One" | "Emilys Candy" | "Engagement" | "Englebert" | "Ephesis" | "Erica One" | "Esteban" | "Estonia" | "Euphoria Script" | "Ewert" | "Exile" | "Explora" | "Faculty Glyphic" | "Farsan" | "Fascinate" | "Fascinate Inline" | "Faster One" | "Fasthand" | "Fauna One" | "Federant" | "Federo" | "Felipa" | "Fenix" | "Festive" | "Finger Paint" | "Fjalla One" | "Fjord One" | "Flavors" | "Fleur De Leah" | "Flow Block" | "Flow Circular" | "Flow Rounded" | "Fontdiner Swanky" | "Forum" | "Francois One" | "Freckle Face" | "Fredericka the Great" | "Freehand" | "Freeman" | "Fresca" | "Frijole" | "Fugaz One" | "Fuggles" | "GFS Didot" | "Ga Maamli" | "Gabriela" | "Gafata" | "Gajraj One" | "Galada" | "Galdeano" | "Galindo" | "Gamja Flower" | "Gasoek One" | "Geostar" | "Geostar Fill" | "Germania One" | "Gideon Roman" | "Gidole" | "Gidugu" | "Gilda Display" | "Girassol" | "Give You Glory" | "Glass Antiqua" | "Gloock" | "Gloria Hallelujah" | "Goblin One" | "Gochi Hand" | "Gotu" | "Goudy Bookletter 1911" | "Gowun Dodum" | "Graduate" | "Grand Hotel" | "Grandiflora One" | "Grape Nuts" | "Gravitas One" | "Great Vibes" | "Grechen Fuemen" | "Grey Qo" | "Griffy" | "Gruppo" | "Gugi" | "Gulzar" | "Gurajada" | "Gveret Levin" | "Habibi" | "Hachi Maru Pop" | "Hammersmith One" | "Hanalei" | "Hanalei Fill" | "Handlee" | "Happy Monkey" | "Headland One" | "Hedvig Letters Sans" | "Henny Penny" | "Herr Von Muellerhoff" | "Hi Melody" | "Hibur Mono" | "Hina Mincho" | "Holtwood One SC" | "Homemade Apple" | "Homenaje" | "Hubballi" | "Huninn" | "Hurricane" | "IM Fell DW Pica SC" | "IM Fell Double Pica SC" | "IM Fell English SC" | "IM Fell French Canon SC" | "IM Fell Great Primer SC" | "Iansui" | "Iceberg" | "Iceland" | "Imperial Script" | "Imprima" | "Inder" | "Indie Flower" | "Ingrid Darling" | "Inspiration" | "Irish Grover" | "Island Moments" | "Isometra" | "Italiana" | "Italianno" | "Itim" | "Jacquard 12" | "Jacquard 12 Charted" | "Jacquard 24" | "Jacquard 24 Charted" | "Jacquarda Bastarda 9" | "Jacquarda Bastarda 9 Charted" | "Jacques Francois" | "Jacques Francois Shadow" | "Jaini" | "Jaini Purva" | "Jersey 10" | "Jersey 10 Charted" | "Jersey 15" | "Jersey 15 Charted" | "Jersey 20" | "Jersey 20 Charted" | "Jersey 25" | "Jersey 25 Charted" | "Jim Nightshade" | "Joan" | "Jockey One" | "Jolly Lodger" | "Jomhuria" | "Jomolhari" | "Joti One" | "Jua" | "Julee" | "Julius Sans One" | "Junge" | "Just Another Hand" | "Just Me Again Down Here" | "Katibeh" | "Kaushan Script" | "Kavivanar" | "Kavoon" | "Kdam Thmor Pro" | "Keania One" | "Kelly Slab" | "Kenia" | "Khmer" | "Kings" | "Kirang Haerang" | "Kite One" | "Knewave" | "Kolker Brush" | "Konkhmer Sleokchher" | "Kosugi" | "Kosugi Maru" | "Kotta One" | "Koulen" | "Kranky" | "Kristi" | "Krona One" | "Kumar One" | "Kumar One Outline" | "Kurale" | "LXGW Marker Gothic" | "La Belle Aurore" | "Lacquer" | "Lakki Reddy" | "Lalezar" | "Lancelot" | "Langar" | "Lavishly Yours" | "League Script" | "Leckerli One" | "Ledger" | "Lemon" | "Libertinus Keyboard" | "Libertinus Math" | "Libertinus Mono" | "Libertinus Serif Display" | "Libre Barcode 128" | "Libre Barcode 128 Text" | "Libre Barcode 39" | "Libre Barcode 39 Extended" | "Libre Barcode 39 Extended Text" | "Libre Barcode 39 Text" | "Libre Barcode EAN13 Text" | "Libre Caslon Display" | "Licorice" | "Lilita One" | "Lily Script One" | "Limelight" | "Liter" | "Liu Jian Mao Cao" | "Lobster" | "Londrina Outline" | "Londrina Shadow" | "Londrina Sketch" | "Long Cang" | "Love Light" | "Love Ya Like A Sister" | "Loved by the King" | "Lovers Quarrel" | "Luckiest Guy" | "Lugrasimo" | "Lumanosimo" | "Lustria" | "Luxurious Roman" | "Luxurious Script" | "Ma Shan Zheng" | "Macondo" | "Macondo Swash Caps" | "Madimi One" | "Maiden Orange" | "Major Mono Display" | "Mako" | "Mallanna" | "Maname" | "Mandali" | "Mansalva" | "Manufacturing Consent" | "Marcellus" | "Marcellus SC" | "Marck Script" | "Margarine" | "Marko One" | "Marmelad" | "Mate SC" | "Matemasie" | "McLaren" | "Mea Culpa" | "Meddon" | "MedievalSharp" | "Medula One" | "Meera Inimai" | "Megrim" | "Meie Script" | "Meow Script" | "Metal" | "Metal Mania" | "Metamorphous" | "Metrophobic" | "Michroma" | "Micro 5" | "Micro 5 Charted" | "Milonga" | "Miltonian" | "Miltonian Tattoo" | "Mingzat" | "Miniver" | "Miss Fajardose" | "Mochiy Pop One" | "Mochiy Pop P One" | "Modak" | "Modern Antiqua" | "Mogra" | "Moirai One" | "Molengo" | "Momo Signature" | "Momo Trust Display" | "Monofett" | "Monomakh" | "Monomaniac One" | "Monoton" | "Monsieur La Doulaise" | "Montaga" | "MonteCarlo" | "Montenegrin Gothic One" | "Montez" | "Moo Lah Lah" | "Mooli" | "Moon Dance" | "Moul" | "Moulpali" | "Mouse Memoirs" | "Mr Bedfort" | "Mr Dafoe" | "Mr De Haviland" | "Mrs Saint Delafield" | "Mrs Sheppards" | "Ms Madi" | "My Soul" | "Mynerve" | "Mystery Quest" | "NTR" | "Nanum Brush Script" | "Nanum Pen Script" | "Neonderthaw" | "Nerko One" | "Neucha" | "New Amsterdam" | "New Rocker" | "New Tegomin" | "Niconne" | "Nixie One" | "Norican" | "Nosifer" | "Notable" | "Nothing You Could Do" | "Noto Color Emoji" | "Noto Music" | "Noto Sans Anatolian Hieroglyphs" | "Noto Sans Avestan" | "Noto Sans Batak" | "Noto Sans Bhaiksuki" | "Noto Sans Brahmi" | "Noto Sans Buginese" | "Noto Sans Buhid" | "Noto Sans Carian" | "Noto Sans Caucasian Albanian" | "Noto Sans Chakma" | "Noto Sans Chorasmian" | "Noto Sans Coptic" | "Noto Sans Cuneiform" | "Noto Sans Cypriot" | "Noto Sans Cypro Minoan" | "Noto Sans Deseret" | "Noto Sans Egyptian Hieroglyphs" | "Noto Sans Elbasan" | "Noto Sans Elymaic" | "Noto Sans Glagolitic" | "Noto Sans Gothic" | "Noto Sans Grantha" | "Noto Sans Hanunoo" | "Noto Sans Hatran" | "Noto Sans Imperial Aramaic" | "Noto Sans Indic Siyaq Numbers" | "Noto Sans Inscriptional Pahlavi" | "Noto Sans Inscriptional Parthian" | "Noto Sans Kaithi" | "Noto Sans Kharoshthi" | "Noto Sans Khojki" | "Noto Sans Khudawadi" | "Noto Sans Lepcha" | "Noto Sans Limbu" | "Noto Sans Linear A" | "Noto Sans Linear B" | "Noto Sans Lycian" | "Noto Sans Lydian" | "Noto Sans Mahajani" | "Noto Sans Mandaic" | "Noto Sans Manichaean" | "Noto Sans Marchen" | "Noto Sans Masaram Gondi" | "Noto Sans Math" | "Noto Sans Mayan Numerals" | "Noto Sans Mende Kikakui" | "Noto Sans Meroitic" | "Noto Sans Miao" | "Noto Sans Modi" | "Noto Sans Mongolian" | "Noto Sans Mro" | "Noto Sans Multani" | "Noto Sans NKo" | "Noto Sans Nabataean" | "Noto Sans Nandinagari" | "Noto Sans Newa" | "Noto Sans Nushu" | "Noto Sans Ogham" | "Noto Sans Old Hungarian" | "Noto Sans Old Italic" | "Noto Sans Old North Arabian" | "Noto Sans Old Permic" | "Noto Sans Old Persian" | "Noto Sans Old Sogdian" | "Noto Sans Old South Arabian" | "Noto Sans Old Turkic" | "Noto Sans Osage" | "Noto Sans Osmanya" | "Noto Sans Pahawh Hmong" | "Noto Sans Palmyrene" | "Noto Sans Pau Cin Hau" | "Noto Sans PhagsPa" | "Noto Sans Phoenician" | "Noto Sans Psalter Pahlavi" | "Noto Sans Rejang" | "Noto Sans Runic" | "Noto Sans Samaritan" | "Noto Sans Saurashtra" | "Noto Sans Sharada" | "Noto Sans Shavian" | "Noto Sans Siddham" | "Noto Sans SignWriting" | "Noto Sans Sogdian" | "Noto Sans Soyombo" | "Noto Sans Sunuwar" | "Noto Sans Syloti Nagri" | "Noto Sans Symbols 2" | "Noto Sans Tagalog" | "Noto Sans Tagbanwa" | "Noto Sans Tai Le" | "Noto Sans Tai Viet" | "Noto Sans Takri" | "Noto Sans Tamil Supplement" | "Noto Sans Tifinagh" | "Noto Sans Tirhuta" | "Noto Sans Ugaritic" | "Noto Sans Vai" | "Noto Sans Wancho" | "Noto Sans Warang Citi" | "Noto Sans Yi" | "Noto Sans Zanabazar Square" | "Noto Serif Ahom" | "Noto Serif Balinese" | "Noto Serif Dives Akuru" | "Noto Serif Dogra" | "Noto Serif Grantha" | "Noto Serif Khitan Small Script" | "Noto Serif Makasar" | "Noto Serif Old Uyghur" | "Noto Serif Ottoman Siyaq" | "Noto Serif Tangut" | "Noto Serif Todhri" | "Noto Znamenny Musical Notation" | "Nova Cut" | "Nova Flat" | "Nova Mono" | "Nova Oval" | "Nova Round" | "Nova Script" | "Nova Slim" | "Nova Square" | "Numans" | "Nuosu SIL" | "Odibee Sans" | "Odor Mean Chey" | "Offside" | "Oi" | "Oldenburg" | "Ole" | "Oooh Baby" | "Oranienbaum" | "Orbit" | "Orelega One" | "Orienta" | "Original Surfer" | "Over the Rainbow" | "Overlock SC" | "Ovo" | "Oxygen Mono" | "PT Mono" | "Pacifico" | "Padyakke Expanded One" | "Palette Mosaic" | "Pangolin" | "Paprika" | "Parisienne" | "Passero One" | "Passions Conflict" | "Pathway Gothic One" | "Patrick Hand" | "Patrick Hand SC" | "Pattaya" | "Patua One" | "Pavanam" | "Paytone One" | "Peddana" | "Peralta" | "Permanent Marker" | "Petemoss" | "Petit Formal Script" | "Piedra" | "Pinyon Script" | "Pirata One" | "Plaster" | "Playball" | "Playwrite AR Guides" | "Playwrite AU NSW Guides" | "Playwrite AU QLD Guides" | "Playwrite AU SA Guides" | "Playwrite AU TAS Guides" | "Playwrite AU VIC Guides" | "Playwrite BE VLG Guides" | "Playwrite BE WAL Guides" | "Playwrite BR Guides" | "Playwrite CA Guides" | "Playwrite CL Guides" | "Playwrite CO Guides" | "Playwrite CU Guides" | "Playwrite CZ Guides" | "Playwrite DE Grund Guides" | "Playwrite DE LA Guides" | "Playwrite DE SAS Guides" | "Playwrite DE VA Guides" | "Playwrite DK Loopet Guides" | "Playwrite DK Uloopet Guides" | "Playwrite ES Deco Guides" | "Playwrite ES Guides" | "Playwrite FR Moderne Guides" | "Playwrite FR Trad Guides" | "Playwrite HR Guides" | "Playwrite HR Lijeva Guides" | "Playwrite HU Guides" | "Playwrite ID Guides" | "Playwrite IE Guides" | "Playwrite IN Guides" | "Playwrite IS Guides" | "Playwrite IT Moderna Guides" | "Playwrite IT Trad Guides" | "Playwrite MX Guides" | "Playwrite NG Modern Guides" | "Playwrite NL Guides" | "Playwrite NO Guides" | "Playwrite NZ Basic Guides" | "Playwrite NZ Guides" | "Playwrite PE Guides" | "Playwrite PL Guides" | "Playwrite PT Guides" | "Playwrite RO Guides" | "Playwrite SK Guides" | "Playwrite TZ Guides" | "Playwrite US Modern Guides" | "Playwrite US Trad Guides" | "Playwrite VN Guides" | "Playwrite ZA Guides" | "Pochaevsk" | "Poetsen One" | "Poiret One" | "Poller One" | "Pompiere" | "Ponnala" | "Ponomar" | "Poor Story" | "Port Lligat Sans" | "Port Lligat Slab" | "Potta One" | "Praise" | "Prata" | "Preahvihear" | "Press Start 2P" | "Princess Sofia" | "Prociono" | "Prosto One" | "Protest Guerrilla" | "Protest Revolution" | "Protest Riot" | "Protest Strike" | "Puppies Play" | "Purple Purse" | "Qahiri" | "Quando" | "Questrial" | "Quintessential" | "Qwigley" | "Racing Sans One" | "Rakkas" | "Raleway Dots" | "Ramabhadra" | "Ramaraja" | "Rammetto One" | "Rampart One" | "Ramsina" | "Ranchers" | "Rancho" | "Rationale" | "Ravi Prakash" | "Redacted" | "Redressed" | "Reem Kufi Ink" | "Reenie Beanie" | "Reggae One" | "Revalia" | "Rhodium Libre" | "Ribeye" | "Ribeye Marrow" | "Righteous" | "Risque" | "Road Rage" | "Rochester" | "Rock 3D" | "Rock Salt" | "RocknRoll One" | "Romanesco" | "Rouge Script" | "Rozha One" | "Rubik 80s Fade" | "Rubik Beastly" | "Rubik Broken Fax" | "Rubik Bubbles" | "Rubik Burned" | "Rubik Dirt" | "Rubik Distressed" | "Rubik Doodle Shadow" | "Rubik Doodle Triangles" | "Rubik Gemstones" | "Rubik Glitch" | "Rubik Glitch Pop" | "Rubik Iso" | "Rubik Lines" | "Rubik Maps" | "Rubik Marker Hatch" | "Rubik Maze" | "Rubik Microbe" | "Rubik Mono One" | "Rubik Moonrocks" | "Rubik Pixels" | "Rubik Puddles" | "Rubik Scribble" | "Rubik Spray Paint" | "Rubik Storm" | "Rubik Vinyl" | "Rubik Wet Paint" | "Ruge Boogie" | "Ruluko" | "Rum Raisin" | "Ruslan Display" | "Russo One" | "Ruthie" | "Rye" | "STIX Two Math" | "Sacramento" | "Sail" | "Salsa" | "Sancreek" | "Sankofa Display" | "Sarina" | "Sassy Frass" | "Satisfy" | "Sawarabi Gothic" | "Sawarabi Mincho" | "Schoolbell" | "Scope One" | "Seaweed Script" | "Secular One" | "Sedan SC" | "Sedgwick Ave" | "Sedgwick Ave Display" | "Sekuya" | "Send Flowers" | "Sevillana" | "Seymour One" | "Shadows Into Light" | "Shadows Into Light Two" | "Shafarik" | "Shalimar" | "Shanti" | "Share Tech" | "Share Tech Mono" | "Shippori Antique" | "Shippori Antique B1" | "Shizuru" | "Shojumaru" | "Short Stack" | "Shrikhand" | "Siemreap" | "Sigmar" | "Sigmar One" | "Single Day" | "Sirin Stencil" | "Sirivennela" | "Six Caps" | "Slabo 13px" | "Slabo 27px" | "Slackey" | "Slackside One" | "Smokum" | "Smooch" | "Smythe" | "Snippet" | "Snowburst One" | "Sofadi One" | "Sofia" | "Solitreo" | "Song Myung" | "Sonsie One" | "Special Elite" | "Special Gothic Condensed One" | "Special Gothic Expanded One" | "Spicy Rice" | "Spinnaker" | "Spirax" | "Splash" | "Squada One" | "Square Peg" | "Sree Krushnadevaraya" | "Sriracha" | "Staatliches" | "Stalemate" | "Stalinist One" | "Stick" | "Stint Ultra Condensed" | "Stint Ultra Expanded" | "Story Script" | "Strait" | "Style Script" | "Stylish" | "Sue Ellen Francisco" | "Suez One" | "Sunshiney" | "Supermercado One" | "Suranna" | "Suravaram" | "Swanky and Moo Moo" | "Syne Mono" | "Syne Tactile" | "Tac One" | "Tagesschrift" | "Tapestry" | "Taprom" | "Tauri" | "Telex" | "Tenali Ramakrishna" | "Tenor Sans" | "Text Me One" | "The Girl Next Door" | "Timmana" | "Titan One" | "Trade Winds" | "Train One" | "Triodion" | "Trocchi" | "Trykker" | "Tulpen One" | "Twinkle Star" | "Ubuntu Condensed" | "Uchen" | "Ultra" | "Uncial Antiqua" | "Underdog" | "Unica One" | "UnifrakturMaguntia" | "Unlock" | "UoqMunThenKhung" | "Updock" | "VT323" | "Vampiro One" | "Varela" | "Varela Round" | "Vast Shadow" | "Viaoda Libre" | "Vibes" | "Vibur" | "Vidaloka" | "Viga" | "Vina Sans" | "Voces" | "Voltaire" | "Vujahday Script" | "WDXL Lubrifont JP N" | "WDXL Lubrifont SC" | "WDXL Lubrifont TC" | "Waiting for the Sunrise" | "Wallpoet" | "Walter Turncoat" | "Warnes" | "Water Brush" | "Waterfall" | "Wellfleet" | "Wendy One" | "Whisper" | "Wire One" | "Yarndings 12" | "Yarndings 12 Charted" | "Yarndings 20" | "Yarndings 20 Charted" | "Yatra One" | "Yellowtail" | "Yeon Sung" | "Yeseva One" | "Yesteryear" | "Yomogi" | "Young Serif" | "Yuji Boku" | "Yuji Hentaigana Akari" | "Yuji Hentaigana Akebono" | "Yuji Mai" | "Yuji Syuku" | "Yusei Magic" | "Yuyu" | "Yuyu Short" | "ZCOOL KuaiLe" | "ZCOOL QingKe HuangYou" | "ZCOOL XiaoWei" | "Zen Antique" | "Zen Antique Soft" | "Zen Dots" | "Zen Kurenaido" | "Zen Tokyo Zoo" | "Zeyada" | "Zhi Mang Xing";
    S10: "Akatab" | "Cinzel" | "Gabarito" | "Golos Text" | "Kedebideri" | "Maven Pro" | "Orbitron" | "Ruda" | "Sarpanch" | "Tirra";
    S100: "Linefont";
    S101: "Literata" | "Source Serif 4";
    S102: "Livvic";
    S103: "Londrina Solid";
    S104: "M PLUS 1p" | "M PLUS Rounded 1c";
    S105: "Merriweather" | "Playfair";
    S106: "Molle";
    S107: "Montagu Slab";
    S108: "Mozilla Headline";
    S109: "Mulish" | "Nunito";
    S11: "Akshar";
    S110: "Nabla";
    S111: "Neuton";
    S112: "Newsreader";
    S113: "Nobile";
    S114: "Nunito Sans";
    S115: "Open Sans";
    S116: "Overlock" | "Playfair Display SC";
    S117: "Pathway Extreme";
    S118: "Playpen Sans" | "Playpen Sans Arabic" | "Playpen Sans Deva" | "Playpen Sans Hebrew" | "Playpen Sans Thai" | "Sora";
    S119: "Playwrite AR" | "Playwrite AU NSW" | "Playwrite AU QLD" | "Playwrite AU SA" | "Playwrite AU TAS" | "Playwrite AU VIC" | "Playwrite BE VLG" | "Playwrite BE WAL" | "Playwrite BR" | "Playwrite CA" | "Playwrite CL" | "Playwrite CO" | "Playwrite CU" | "Playwrite CZ" | "Playwrite DE Grund" | "Playwrite DE LA" | "Playwrite DE SAS" | "Playwrite DE VA" | "Playwrite DK Loopet" | "Playwrite DK Uloopet" | "Playwrite ES" | "Playwrite ES Deco" | "Playwrite FR Moderne" | "Playwrite FR Trad" | "Playwrite HR" | "Playwrite HR Lijeva" | "Playwrite HU" | "Playwrite ID" | "Playwrite IE" | "Playwrite IN" | "Playwrite IS" | "Playwrite IT Moderna" | "Playwrite IT Trad" | "Playwrite MX" | "Playwrite NG Modern" | "Playwrite NL" | "Playwrite NO" | "Playwrite NZ" | "Playwrite NZ Basic" | "Playwrite PE" | "Playwrite PL" | "Playwrite PT" | "Playwrite RO" | "Playwrite SK" | "Playwrite TZ" | "Playwrite US Modern" | "Playwrite US Trad" | "Playwrite VN" | "Playwrite ZA";
    S12: "Akt" | "Alexandria" | "Catamaran" | "Cause" | "Comme" | "Encode Sans Condensed" | "Encode Sans Expanded" | "Encode Sans Semi Condensed" | "Encode Sans Semi Expanded" | "Estedad" | "Foldit" | "Gothic A1" | "Grenze Gotisch" | "Hahmlet" | "Hanuman" | "Heebo" | "Kripa" | "League Spartan" | "Lexend" | "Lexend Deca" | "Lexend Exa" | "Lexend Giga" | "Lexend Mega" | "Lexend Peta" | "Lexend Tera" | "Lexend Zetta" | "M PLUS 1" | "M PLUS 2" | "M PLUS U" | "Murecho" | "Nata Sans" | "Nokora" | "Noto Kufi Arabic" | "Noto Rashi Hebrew" | "Noto Sans Canadian Aboriginal" | "Noto Sans Cham" | "Noto Sans Cherokee" | "Noto Sans HK" | "Noto Sans JP" | "Noto Sans KR" | "Noto Sans Meetei Mayek" | "Noto Sans SC" | "Noto Sans Symbols" | "Noto Sans Syriac" | "Noto Sans Syriac Eastern" | "Noto Sans Syriac Western" | "Noto Sans TC" | "Noto Sans Thaana" | "Noto Serif Gujarati" | "Noto Serif Gurmukhi" | "Noto Serif Kannada" | "Noto Serif Malayalam" | "Noto Serif Myanmar" | "Noto Serif Telugu" | "Noto Serif Tibetan" | "Onest" | "Outfit" | "Roboto Slab" | "Saira Condensed" | "Saira Extra Condensed" | "Saira Semi Condensed" | "Smooch Sans" | "Vazirmatn";
    S120: "Playwrite AT" | "Playwrite GB J" | "Playwrite GB S";
    S121: "Radio Canada";
    S122: "Readex Pro";
    S123: "Recursive";
    S124: "Roboto Flex";
    S125: "Roboto Serif";
    S126: "Sansita";
    S127: "Science Gothic";
    S128: "Shantell Sans";
    S129: "Signika";
    S13: "Alan Sans" | "Alyamama" | "Darker Grotesque" | "Frank Ruhl Libre" | "Inknut Antiqua" | "Matangi" | "Merienda" | "Phudu" | "Sansita Swashed";
    S130: "Simonetta";
    S131: "Sixtyfour" | "Workbench";
    S132: "Sixtyfour Convergence";
    S133: "Sofia Sans" | "Sofia Sans Condensed" | "Sofia Sans Extra Condensed" | "Sofia Sans Semi Condensed" | "Ysabeau" | "Ysabeau Infant" | "Ysabeau Office";
    S134: "Solway";
    S135: "Sono";
    S136: "Strichpunkt Sans" | "Tektur";
    S137: "Sunflower";
    S138: "Tajawal";
    S139: "TikTok Sans";
    S14: "Albert Sans" | "Aleo" | "Alumni Sans" | "Alumni Sans SC" | "Azeret Mono" | "Barlow" | "Barlow Condensed" | "Barlow Semi Condensed" | "Be Vietnam Pro" | "Bitter" | "Chivo" | "Chivo Mono" | "Clarity City" | "Elms Sans" | "Epilogue" | "Exo" | "Exo 2" | "Finlandica Headline" | "Finlandica Text" | "Fira Sans" | "Fira Sans Condensed" | "Fira Sans Extra Condensed" | "Gantari" | "Geist" | "Geist Mono" | "Genos" | "Grandstander" | "Grenze" | "Hanken Grotesk" | "Inter Tight" | "Jost" | "Kanit" | "Labrada" | "Libre Franklin" | "Montserrat" | "Montserrat Alternates" | "Montserrat Underline" | "MuseoModerno" | "Overpass" | "Petrona" | "Poppins" | "Prompt" | "Public Sans" | "REM" | "Raleway" | "Roboto Condensed" | "Rokkitt" | "SUSE" | "Taviraj" | "Tomorrow" | "Trirong" | "Urbanist" | "Valley Sans" | "Work Sans" | "Xmind";
    S140: "Tilt Neon" | "Tilt Prism" | "Tilt Warp";
    S141: "Tiny5" | "Tiny5 Duo";
    S142: "Titillium Web";
    S143: "Truculenta";
    S144: "Turret Road";
    S145: "Ubuntu Sans";
    S146: "UnifrakturCook";
    S147: "Vesper Libre";
    S148: "Vollkorn SC";
    S149: "Wavefont";
    S15: "Alegreya" | "Besley" | "Kufam" | "Playfair Display" | "Schibsted Grotesk" | "Vollkorn" | "Wittgenstein";
    S150: "Yantramanav";
    S151: "Ysabeau SC";
    S152: "Zain";
    S153: "Zen Kaku Gothic Antique" | "Zen Kaku Gothic New" | "Zen Maru Gothic";
    S154: "Zen Old Mincho";
    S16: "Alegreya SC";
    S17: "Alegreya Sans" | "Alegreya Sans SC";
    S18: "Alkatra" | "BJCree" | "Baskervville SC" | "Buenard" | "Cabin Condensed" | "Caveat" | "Dancing Script" | "Domine" | "Edu AU VIC WA NT Arrows" | "Edu AU VIC WA NT Dots" | "Edu AU VIC WA NT Guides" | "Edu AU VIC WA NT Hand" | "Edu AU VIC WA NT Pre" | "Edu NSW ACT Cursive" | "Edu NSW ACT Foundation" | "Edu NSW ACT Hand Pre" | "Edu QLD Beginner" | "Edu QLD Hand" | "Edu SA Beginner" | "Edu SA Hand" | "Edu TAS Beginner" | "Edu VIC WA NT Beginner" | "Edu VIC WA NT Hand" | "Edu VIC WA NT Hand Pre" | "El Messiri" | "Enriqueta" | "Harmattan" | "Kameron" | "Kanchenjunga" | "Kay Pho Du" | "Markazi Text" | "Miriam Libre" | "Mirza" | "Monda" | "Noto Naskh Arabic" | "Noto Nastaliq Urdu" | "Noto Sans Adlam" | "Noto Sans Adlam Unjoined" | "Noto Sans Balinese" | "Noto Sans Bamum" | "Noto Sans Bassa Vah" | "Noto Sans Gunjala Gondi" | "Noto Sans Hanifi Rohingya" | "Noto Sans Javanese" | "Noto Sans Kawi" | "Noto Sans Kayah Li" | "Noto Sans Lisu" | "Noto Sans Medefaidrin" | "Noto Sans NKo Unjoined" | "Noto Sans Nag Mundari" | "Noto Sans New Tai Lue" | "Noto Sans Ol Chiki" | "Noto Sans Sora Sompeng" | "Noto Sans Sundanese" | "Noto Sans Tai Tham" | "Noto Sans Tangsa" | "Noto Sans Vithkuqi" | "Noto Serif Khojki" | "Noto Serif NP Hmong" | "Noto Serif Oriya" | "Noto Serif Toto" | "Noto Serif Vithkuqi" | "Noto Serif Yezidi" | "Palanquin Dark" | "Parastoo" | "Pixelify Sans" | "Reem Kufi" | "Reem Kufi Fun" | "Ruwudu" | "Scheherazade New";
    S19: "Almarai";
    S2: "AR One Sans";
    S20: "Almendra" | "Amaranth" | "Amiri" | "Andika" | "Anonymous Pro" | "Arsenal" | "Arsenal SC" | "Arvo" | "Atkinson Hyperlegible" | "B612" | "B612 Mono" | "Balsamiq Sans" | "Bona Nova" | "Bona Nova SC" | "Caladea" | "Cambay" | "Cantarell" | "Cardo" | "Carlito" | "Caudex" | "Charis SIL" | "Courier Prime" | "Cousine" | "Economica" | "GFS Neohellenic" | "Gentium Book Plus" | "Gentium Plus" | "Gudea" | "Istok Web" | "Judson" | "Lekton" | "Libertinus Sans" | "Libre Caslon Text" | "Lobster Two" | "Marvel" | "Noticia Text" | "Old Standard TT" | "PT Sans" | "PT Serif" | "Philosopher" | "Puritan" | "Quantico" | "Quattrocento Sans" | "Rambla" | "Redaction" | "Redaction 10" | "Redaction 100" | "Redaction 20" | "Redaction 35" | "Redaction 50" | "Redaction 70" | "Scada" | "Share" | "Space Mono" | "Thasadith" | "Tinos" | "Trochut" | "Tuffy" | "Ubuntu Mono" | "Unna" | "Volkhov";
    S21: "Amarna" | "IBM Plex Mono" | "IBM Plex Sans Condensed" | "IBM Plex Serif" | "Josefin Sans" | "Josefin Slab" | "Kantumruy Pro" | "Lilex" | "Roboto Mono" | "Victor Mono";
    S22: "Amiko" | "Belanosima";
    S23: "Ancizar Sans";
    S24: "Ancizar Serif" | "Epunda Sans" | "Epunda Slab" | "Figtree" | "Geom" | "Red Hat Display" | "Rubik" | "Winky Rough" | "Winky Sans";
    S25: "Andada Pro" | "EB Garamond" | "Proza Libre" | "Rethink Sans" | "Teachers" | "Wix Madefor Text";
    S26: "Anek Bangla" | "Anek Devanagari" | "Anek Gujarati" | "Anek Gurmukhi" | "Anek Kannada" | "Anek Latin" | "Anek Malayalam" | "Anek Odia" | "Anek Tamil" | "Anek Telugu" | "Firjar" | "Martian Mono" | "Trispace";
    S27: "Asap Condensed" | "Chiron Hei HK" | "Chiron Sung HK" | "Crimson Pro" | "Lisu Bosa" | "Reddit Sans" | "SN Pro" | "Savate" | "Source Code Pro" | "Source Sans 3" | "Zalando Sans Expanded" | "Zalando Sans SemiExpanded";
    S28: "Assistant" | "Changa" | "Dosis" | "Fustat" | "Gemunu Libre" | "Geomini" | "Lateef" | "Manrope" | "Momo Trust Sans" | "Mukta" | "Mukta Mahee" | "Mukta Malar" | "Mukta Vaani" | "National Park" | "Ojuju" | "Oxanium" | "Stick No Bills";
    S29: "Asta Sans" | "Funnel Display" | "Moderustic" | "Parkinsans";
    S3: "Abhaya Libre" | "Anaheim" | "Baloo 2" | "Baloo Bhai 2" | "Baloo Bhaijaan 2" | "Baloo Bhaina 2" | "Baloo Chettan 2" | "Baloo Da 2" | "Baloo Paaji 2" | "Baloo Tamma 2" | "Baloo Tammudu 2" | "Baloo Thambi 2" | "Eczar" | "Namdhinggo" | "Narnoor" | "Podkova" | "Sen" | "Shippori Mincho" | "Shippori Mincho B1" | "Syne" | "TASA Explorer" | "TASA Orbiter" | "Tillana" | "Wix Madefor Display";
    S30: "Athiti" | "Maitree" | "Mitr" | "Mozilla Text" | "Oswald" | "Pridi" | "Stack Sans Headline" | "Stack Sans Notch" | "Stack Sans Text" | "Yaldevi" | "Yanone Kaffeesatz";
    S31: "Atkinson Hyperlegible Mono" | "Atkinson Hyperlegible Next" | "Karla" | "Plus Jakarta Sans" | "Scoutie Sans" | "Spectral" | "Spectral SC";
    S32: "Atma" | "Comfortaa" | "Cormorant SC" | "Cormorant Unicase" | "Cormorant Upright" | "Fira Code" | "Halant" | "Hind" | "Hind Guntur" | "Hind Madurai" | "Hind Mysuru" | "Hind Siliguri" | "Hind Vadodara" | "Jura" | "Karma" | "Khand" | "Kreon" | "Laila" | "Lemonada" | "Marhey" | "Noto Emoji" | "Noto Traditional Nushu" | "Overpass Mono" | "Pontano Sans" | "Quicksand" | "Rajdhani" | "Red Rose" | "Signika Negative" | "Space Grotesk" | "Spline Sans" | "Teko" | "Tsukimi Rounded" | "Varta";
    S33: "Averia Libre" | "Averia Sans Libre" | "Averia Serif Libre" | "Bellota" | "Bellota Text" | "Comic Neue" | "Inria Sans" | "Inria Serif" | "Sansation";
    S34: "Bai Jamjuree" | "Cascadia Code" | "Cascadia Mono" | "Fahkwang" | "KoHo" | "Kodchasan" | "Krub" | "Mali" | "Niramit";
    S35: "Ballet" | "Hedvig Letters Serif" | "Jaro";
    S36: "Battambang" | "Koh Santepheap" | "Suwannaphum";
    S37: "Beiruti" | "Chiron GoRound TC" | "Mada" | "Noto Serif HK" | "Noto Serif Hentaigana" | "Noto Serif JP" | "Noto Serif KR" | "Noto Serif SC" | "Noto Serif TC" | "Reddit Mono" | "Reddit Sans Condensed" | "Unbounded";
    S38: "BenchNine" | "Dongle" | "Gaegu" | "Kalam" | "Karantina" | "LXGW WenKai Mono TC" | "LXGW WenKai TC" | "Oxygen" | "Redacted Script" | "Rowdies" | "Sulphur Point";
    S39: "Big Shoulders" | "Big Shoulders Inline" | "Big Shoulders Stencil" | "Imbue";
    S4: "Advent Pro" | "Anybody" | "Archivo" | "Asap" | "Asap Sharp" | "Georama" | "Noto Sans" | "Noto Sans Display" | "Noto Serif" | "Noto Serif Display" | "Noto Serif Tamil" | "Pliant" | "Roboto" | "Saira" | "Saira Stencil" | "Sour Gummy" | "Tourney";
    S40: "BioRhyme";
    S41: "BioRhyme Expanded";
    S42: "Birthstone Bounce" | "WindSong";
    S43: "Biryani" | "Martel" | "Martel Sans";
    S44: "Bitcount" | "Bitcount Grid Double" | "Bitcount Grid Single" | "Bitcount Prop Double" | "Bitcount Prop Single" | "Bitcount Single";
    S45: "Bitcount Grid Double Ink" | "Bitcount Grid Single Ink" | "Bitcount Ink" | "Bitcount Prop Double Ink" | "Bitcount Prop Single Ink" | "Bitcount Single Ink";
    S46: "Blinker";
    S47: "Bodoni Moda" | "Bodoni Moda SC";
    S48: "Bricolage Grotesque";
    S49: "Buda";
    S5: "Afacad" | "Archivo Narrow" | "Arimo" | "Baskervville" | "Brygada 1918" | "Castoro" | "Cuprum" | "Expletus Sans" | "Familjen Grotesk" | "Gelasio" | "Ibarra Real Nova" | "Libre Baskerville" | "Libre Bodoni" | "Libre Caslon Condensed" | "Lora" | "Miranda Sans" | "Poltawski Nowy" | "Radio Canada Big" | "STIX Two Text" | "Sometype Mono" | "Ubuntu Sans Mono";
    S50: "Cabin" | "Instrument Sans";
    S51: "Cairo" | "Cairo Play";
    S52: "Chakra Petch" | "Cormorant" | "Cormorant Garamond" | "Cormorant Infant" | "Dai Banna SIL" | "Inclusive Sans" | "Intel One Mono" | "Mohave" | "Rasa" | "Red Hat Mono" | "Red Hat Text" | "Rosario" | "Spline Sans Mono" | "Vend Sans" | "Yrsa" | "Zilla Slab";
    S53: "Chathura";
    S54: "Cinzel Decorative" | "Passion One" | "Tienne";
    S55: "Climate Crisis";
    S56: "Coda" | "Sniglet";
    S57: "Codystar" | "Flamenco" | "Kapakana" | "Stoke";
    S58: "Commissioner";
    S59: "Coustard" | "Elsie" | "Elsie Swash Caps";
    S6: "Afacad Flux";
    S60: "Crimson Text" | "Libertinus Serif";
    S61: "DM Mono";
    S62: "DM Sans";
    S63: "Danfo" | "Geist Pixel";
    S64: "Datatype" | "Encode Sans" | "Encode Sans SC" | "Noto Sans Arabic" | "Noto Sans Armenian" | "Noto Sans Bengali" | "Noto Sans Devanagari" | "Noto Sans Ethiopic" | "Noto Sans Georgian" | "Noto Sans Gujarati" | "Noto Sans Gurmukhi" | "Noto Sans Hebrew" | "Noto Sans Kannada" | "Noto Sans Khmer" | "Noto Sans Lao" | "Noto Sans Lao Looped" | "Noto Sans Malayalam" | "Noto Sans Mono" | "Noto Sans Myanmar" | "Noto Sans Oriya" | "Noto Sans Sinhala" | "Noto Sans Tamil" | "Noto Sans Telugu" | "Noto Sans Thai" | "Noto Sans Thai Looped" | "Noto Serif Armenian" | "Noto Serif Bengali" | "Noto Serif Devanagari" | "Noto Serif Ethiopic" | "Noto Serif Georgian" | "Noto Serif Hebrew" | "Noto Serif Khmer" | "Noto Serif Lao" | "Noto Serif Sinhala" | "Noto Serif Thai";
    S65: "David Libre" | "Fira Mono" | "Gupter" | "Kaisei Decol" | "Kaisei HarunoUmi" | "Kaisei Opti";
    S66: "Doto";
    S67: "DynaPuff" | "Special Gothic";
    S68: "Farro";
    S69: "Faustina" | "Funnel Sans" | "Host Grotesk" | "Manuale" | "Merriweather Sans" | "Platypi";
    S7: "Agdasima" | "Alef" | "Allan" | "Amatic SC" | "Amita" | "Annapurna SIL" | "Aref Ruqaa" | "Aref Ruqaa Ink" | "Arya" | "Astloch" | "Asul" | "BIZ UDGothic" | "BIZ UDMincho" | "BIZ UDPGothic" | "BIZ UDPMincho" | "Brawler" | "Cabin Sketch" | "Charm" | "Charmonman" | "Cherry Swash" | "Comic Relief" | "Content" | "Corben" | "Corinthia" | "Cossette Texte" | "Cossette Titre" | "Delius Unicase" | "Fuzzy Bubbles" | "Glegoo" | "Goldman" | "Gorditas" | "Gowun Batang" | "Gwendolyn" | "Inika" | "Jaldi" | "Kadwa" | "Karla Tamil Inclined" | "Karla Tamil Upright" | "Lunasima" | "Lusitana" | "Magra" | "Mina" | "Mountains of Christmas" | "Nanum Gothic Coding" | "News Cycle" | "Noto Sans Duployan" | "Oleo Script" | "Oleo Script Swash Caps" | "PT Sans Caption" | "PT Sans Narrow" | "Padauk" | "Phetsarath" | "Play" | "Pragati Narrow" | "Quattrocento" | "Qwitcher Grypen" | "Ranga" | "Rufina" | "Sahitya" | "Sarala" | "Silkscreen" | "Sintony" | "Skranji" | "Srisakdi" | "Stardos Stencil" | "Sumana" | "Sura" | "Syncopate" | "Tai Heritage Pro" | "Tangerine" | "The Nautigal" | "Unkempt" | "Zilla Slab Highlight";
    S70: "Fraunces";
    S71: "Fredoka";
    S72: "Gayathri" | "Manjari";
    S73: "Geologica";
    S74: "Glory" | "JetBrains Mono" | "K2D" | "SUSE Mono" | "Sarabun";
    S75: "Gluten";
    S76: "Google Sans";
    S77: "Google Sans Code";
    S78: "Google Sans Flex";
    S79: "Handjet";
    S8: "Agrave Pro" | "Antonio" | "Anuphan" | "Arima" | "IBM Plex Sans Arabic" | "IBM Plex Sans Devanagari" | "IBM Plex Sans Hebrew" | "IBM Plex Sans JP" | "IBM Plex Sans KR" | "IBM Plex Sans Thai" | "IBM Plex Sans Thai Looped" | "M PLUS 1 Code" | "Menbere" | "Palanquin";
    S80: "Hepta Slab";
    S81: "Honk";
    S82: "Hubot Sans" | "Mona Sans" | "Zalando Sans";
    S83: "IBM Plex Sans";
    S84: "Idiqlat";
    S85: "Inconsolata";
    S86: "Inter" | "Piazzolla" | "Texturina";
    S87: "Iosevka Charon" | "Iosevka Charon Mono" | "Ubuntu";
    S88: "Kaisei Tokumin";
    S89: "Kalnia" | "Kalnia Glaze" | "M PLUS Code Latin";
    S9: "Agu Display" | "Kablammo";
    S90: "Khula";
    S91: "Kiwi Maru";
    S92: "Klee One";
    S93: "Kode Mono";
    S94: "Kulim Park";
    S95: "Kumbh Sans";
    S96: "LINE Seed JP";
    S97: "Lato";
    S98: "League Gothic";
    S99: "Life Savers" | "Nanum Gothic" | "Nanum Myeongjo";
}

interface GoogleFontShapes {
    S0: {
        weight: 400;
        style: "italic" | "normal";
        axis: never;
    };
    S1: {
        weight: 400;
        style: "normal";
        axis: never;
    };
    S10: {
        weight: 400 | 500 | 600 | 700 | 800 | 900;
        style: "normal";
        axis: never;
    };
    S100: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900 | 1000;
        style: "normal";
        axis: "wdth";
    };
    S101: {
        weight: 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "italic" | "normal";
        axis: "opsz";
    };
    S102: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 900;
        style: "italic" | "normal";
        axis: never;
    };
    S103: {
        weight: 100 | 300 | 400 | 900;
        style: "normal";
        axis: never;
    };
    S104: {
        weight: 100 | 300 | 400 | 500 | 700 | 800 | 900;
        style: "normal";
        axis: never;
    };
    S105: {
        weight: 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "italic" | "normal";
        axis: "opsz" | "wdth";
    };
    S106: {
        weight: 400;
        style: "italic";
        axis: never;
    };
    S107: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700;
        style: "normal";
        axis: "opsz";
    };
    S108: {
        weight: 200 | 300 | 400 | 500 | 600 | 700;
        style: "normal";
        axis: "wdth";
    };
    S109: {
        weight: 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900 | 1000;
        style: "italic" | "normal";
        axis: never;
    };
    S11: {
        weight: 300 | 400 | 500 | 600 | 700;
        style: "normal";
        axis: "CTRS";
    };
    S110: {
        weight: 400;
        style: "normal";
        axis: "EDPT" | "EHLT";
    };
    S111: {
        weight: 200 | 300 | 400 | 700 | 800;
        style: "italic" | "normal";
        axis: never;
    };
    S112: {
        weight: 200 | 300 | 400 | 500 | 600 | 700 | 800;
        style: "italic" | "normal";
        axis: "opsz";
    };
    S113: {
        weight: 400 | 500 | 700;
        style: "italic" | "normal";
        axis: never;
    };
    S114: {
        weight: 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900 | 1000;
        style: "italic" | "normal";
        axis: "YTLC" | "opsz" | "wdth";
    };
    S115: {
        weight: 300 | 400 | 500 | 600 | 700 | 800;
        style: "italic" | "normal";
        axis: "wdth";
    };
    S116: {
        weight: 400 | 700 | 900;
        style: "italic" | "normal";
        axis: never;
    };
    S117: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "italic" | "normal";
        axis: "opsz" | "wdth";
    };
    S118: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800;
        style: "normal";
        axis: never;
    };
    S119: {
        weight: 100 | 200 | 300 | 400;
        style: "normal";
        axis: never;
    };
    S12: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "normal";
        axis: never;
    };
    S120: {
        weight: 100 | 200 | 300 | 400;
        style: "italic" | "normal";
        axis: never;
    };
    S121: {
        weight: 300 | 400 | 500 | 600 | 700;
        style: "italic" | "normal";
        axis: "wdth";
    };
    S122: {
        weight: 200 | 300 | 400 | 500 | 600 | 700;
        style: "normal";
        axis: "HEXP";
    };
    S123: {
        weight: 300 | 400 | 500 | 600 | 700 | 800 | 900 | 1000;
        style: "normal";
        axis: "CASL" | "CRSV" | "MONO" | "slnt";
    };
    S124: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900 | 1000;
        style: "normal";
        axis: "GRAD" | "XOPQ" | "XTRA" | "YOPQ" | "YTAS" | "YTDE" | "YTFI" | "YTLC" | "YTUC" | "opsz" | "slnt" | "wdth";
    };
    S125: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "italic" | "normal";
        axis: "GRAD" | "opsz" | "wdth";
    };
    S126: {
        weight: 400 | 700 | 800 | 900;
        style: "italic" | "normal";
        axis: never;
    };
    S127: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "normal";
        axis: "CTRS" | "slnt" | "wdth";
    };
    S128: {
        weight: 300 | 400 | 500 | 600 | 700 | 800;
        style: "italic" | "normal";
        axis: "BNCE" | "INFM" | "SPAC";
    };
    S129: {
        weight: 300 | 400 | 500 | 600 | 700;
        style: "normal";
        axis: "GRAD";
    };
    S13: {
        weight: 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "normal";
        axis: never;
    };
    S130: {
        weight: 400 | 900;
        style: "italic" | "normal";
        axis: never;
    };
    S131: {
        weight: 400;
        style: "normal";
        axis: "BLED" | "SCAN";
    };
    S132: {
        weight: 400;
        style: "normal";
        axis: "BLED" | "SCAN" | "XELA" | "YELA";
    };
    S133: {
        weight: 1 | 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900 | 1000;
        style: "italic" | "normal";
        axis: never;
    };
    S134: {
        weight: 300 | 400 | 500 | 700 | 800;
        style: "normal";
        axis: never;
    };
    S135: {
        weight: 200 | 300 | 400 | 500 | 600 | 700 | 800;
        style: "normal";
        axis: "MONO";
    };
    S136: {
        weight: 400 | 500 | 600 | 700 | 800 | 900;
        style: "normal";
        axis: "wdth";
    };
    S137: {
        weight: 300 | 500 | 700;
        style: "normal";
        axis: never;
    };
    S138: {
        weight: 200 | 300 | 400 | 500 | 700 | 800 | 900;
        style: "normal";
        axis: never;
    };
    S139: {
        weight: 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "normal";
        axis: "opsz" | "slnt" | "wdth";
    };
    S14: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "italic" | "normal";
        axis: never;
    };
    S140: {
        weight: 400;
        style: "normal";
        axis: "XROT" | "YROT";
    };
    S141: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "italic" | "normal";
        axis: "BLED" | "JITT" | "ROND" | "wdth";
    };
    S142: {
        weight: 200 | 300 | 400 | 600 | 700 | 900;
        style: "italic" | "normal";
        axis: never;
    };
    S143: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "normal";
        axis: "opsz" | "wdth";
    };
    S144: {
        weight: 200 | 300 | 400 | 500 | 700 | 800;
        style: "normal";
        axis: never;
    };
    S145: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800;
        style: "italic" | "normal";
        axis: "wdth";
    };
    S146: {
        weight: 700;
        style: "normal";
        axis: never;
    };
    S147: {
        weight: 400 | 500 | 700 | 900;
        style: "normal";
        axis: never;
    };
    S148: {
        weight: 400 | 600 | 700 | 900;
        style: "normal";
        axis: never;
    };
    S149: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900 | 1000;
        style: "normal";
        axis: "ROND" | "YELA";
    };
    S15: {
        weight: 400 | 500 | 600 | 700 | 800 | 900;
        style: "italic" | "normal";
        axis: never;
    };
    S150: {
        weight: 100 | 300 | 400 | 500 | 700 | 900;
        style: "normal";
        axis: never;
    };
    S151: {
        weight: 1 | 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900 | 1000;
        style: "normal";
        axis: never;
    };
    S152: {
        weight: 200 | 300 | 400 | 700 | 800 | 900;
        style: "italic" | "normal";
        axis: never;
    };
    S153: {
        weight: 300 | 400 | 500 | 700 | 900;
        style: "normal";
        axis: never;
    };
    S154: {
        weight: 400 | 500 | 600 | 700 | 900;
        style: "normal";
        axis: never;
    };
    S16: {
        weight: 400 | 500 | 700 | 800 | 900;
        style: "italic" | "normal";
        axis: never;
    };
    S17: {
        weight: 100 | 300 | 400 | 500 | 700 | 800 | 900;
        style: "italic" | "normal";
        axis: never;
    };
    S18: {
        weight: 400 | 500 | 600 | 700;
        style: "normal";
        axis: never;
    };
    S19: {
        weight: 300 | 400 | 700 | 800;
        style: "normal";
        axis: never;
    };
    S2: {
        weight: 400 | 500 | 600 | 700;
        style: "normal";
        axis: "ARRR";
    };
    S20: {
        weight: 400 | 700;
        style: "italic" | "normal";
        axis: never;
    };
    S21: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700;
        style: "italic" | "normal";
        axis: never;
    };
    S22: {
        weight: 400 | 600 | 700;
        style: "normal";
        axis: never;
    };
    S23: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900 | 1000;
        style: "italic" | "normal";
        axis: never;
    };
    S24: {
        weight: 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "italic" | "normal";
        axis: never;
    };
    S25: {
        weight: 400 | 500 | 600 | 700 | 800;
        style: "italic" | "normal";
        axis: never;
    };
    S26: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800;
        style: "normal";
        axis: "wdth";
    };
    S27: {
        weight: 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "italic" | "normal";
        axis: never;
    };
    S28: {
        weight: 200 | 300 | 400 | 500 | 600 | 700 | 800;
        style: "normal";
        axis: never;
    };
    S29: {
        weight: 300 | 400 | 500 | 600 | 700 | 800;
        style: "normal";
        axis: never;
    };
    S3: {
        weight: 400 | 500 | 600 | 700 | 800;
        style: "normal";
        axis: never;
    };
    S30: {
        weight: 200 | 300 | 400 | 500 | 600 | 700;
        style: "normal";
        axis: never;
    };
    S31: {
        weight: 200 | 300 | 400 | 500 | 600 | 700 | 800;
        style: "italic" | "normal";
        axis: never;
    };
    S32: {
        weight: 300 | 400 | 500 | 600 | 700;
        style: "normal";
        axis: never;
    };
    S33: {
        weight: 300 | 400 | 700;
        style: "italic" | "normal";
        axis: never;
    };
    S34: {
        weight: 200 | 300 | 400 | 500 | 600 | 700;
        style: "italic" | "normal";
        axis: never;
    };
    S35: {
        weight: 400;
        style: "normal";
        axis: "opsz";
    };
    S36: {
        weight: 100 | 300 | 400 | 700 | 900;
        style: "normal";
        axis: never;
    };
    S37: {
        weight: 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "normal";
        axis: never;
    };
    S38: {
        weight: 300 | 400 | 700;
        style: "normal";
        axis: never;
    };
    S39: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "normal";
        axis: "opsz";
    };
    S4: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "italic" | "normal";
        axis: "wdth";
    };
    S40: {
        weight: 200 | 300 | 400 | 500 | 600 | 700 | 800;
        style: "normal";
        axis: "wdth";
    };
    S41: {
        weight: 200 | 300 | 400 | 700 | 800;
        style: "normal";
        axis: never;
    };
    S42: {
        weight: 400 | 500;
        style: "normal";
        axis: never;
    };
    S43: {
        weight: 200 | 300 | 400 | 600 | 700 | 800 | 900;
        style: "normal";
        axis: never;
    };
    S44: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "normal";
        axis: "CRSV" | "ELSH" | "ELXP" | "slnt";
    };
    S45: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "normal";
        axis: "CRSV" | "ELSH" | "ELXP" | "SZP1" | "SZP2" | "XPN1" | "XPN2" | "YPN1" | "YPN2" | "slnt";
    };
    S46: {
        weight: 100 | 200 | 300 | 400 | 600 | 700 | 800 | 900;
        style: "normal";
        axis: never;
    };
    S47: {
        weight: 400 | 500 | 600 | 700 | 800 | 900;
        style: "italic" | "normal";
        axis: "opsz";
    };
    S48: {
        weight: 200 | 300 | 400 | 500 | 600 | 700 | 800;
        style: "normal";
        axis: "opsz" | "wdth";
    };
    S49: {
        weight: 300;
        style: "normal";
        axis: never;
    };
    S5: {
        weight: 400 | 500 | 600 | 700;
        style: "italic" | "normal";
        axis: never;
    };
    S50: {
        weight: 400 | 500 | 600 | 700;
        style: "italic" | "normal";
        axis: "wdth";
    };
    S51: {
        weight: 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900 | 1000;
        style: "normal";
        axis: "slnt";
    };
    S52: {
        weight: 300 | 400 | 500 | 600 | 700;
        style: "italic" | "normal";
        axis: never;
    };
    S53: {
        weight: 100 | 300 | 400 | 700 | 800;
        style: "normal";
        axis: never;
    };
    S54: {
        weight: 400 | 700 | 900;
        style: "normal";
        axis: never;
    };
    S55: {
        weight: 400;
        style: "normal";
        axis: "YEAR";
    };
    S56: {
        weight: 400 | 800;
        style: "normal";
        axis: never;
    };
    S57: {
        weight: 300 | 400;
        style: "normal";
        axis: never;
    };
    S58: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "normal";
        axis: "FLAR" | "VOLM" | "slnt";
    };
    S59: {
        weight: 400 | 900;
        style: "normal";
        axis: never;
    };
    S6: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900 | 1000;
        style: "normal";
        axis: "slnt";
    };
    S60: {
        weight: 400 | 600 | 700;
        style: "italic" | "normal";
        axis: never;
    };
    S61: {
        weight: 300 | 400 | 500;
        style: "italic" | "normal";
        axis: never;
    };
    S62: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900 | 1000;
        style: "italic" | "normal";
        axis: "opsz";
    };
    S63: {
        weight: 400;
        style: "normal";
        axis: "ELSH";
    };
    S64: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "normal";
        axis: "wdth";
    };
    S65: {
        weight: 400 | 500 | 700;
        style: "normal";
        axis: never;
    };
    S66: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "normal";
        axis: "ROND";
    };
    S67: {
        weight: 400 | 500 | 600 | 700;
        style: "normal";
        axis: "wdth";
    };
    S68: {
        weight: 300 | 400 | 500 | 700;
        style: "normal";
        axis: never;
    };
    S69: {
        weight: 300 | 400 | 500 | 600 | 700 | 800;
        style: "italic" | "normal";
        axis: never;
    };
    S7: {
        weight: 400 | 700;
        style: "normal";
        axis: never;
    };
    S70: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "italic" | "normal";
        axis: "SOFT" | "WONK" | "opsz";
    };
    S71: {
        weight: 300 | 400 | 500 | 600 | 700;
        style: "normal";
        axis: "wdth";
    };
    S72: {
        weight: 100 | 400 | 700;
        style: "normal";
        axis: never;
    };
    S73: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "normal";
        axis: "CRSV" | "SHRP" | "slnt";
    };
    S74: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800;
        style: "italic" | "normal";
        axis: never;
    };
    S75: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "normal";
        axis: "slnt";
    };
    S76: {
        weight: 400 | 500 | 600 | 700;
        style: "italic" | "normal";
        axis: "GRAD" | "opsz";
    };
    S77: {
        weight: 300 | 400 | 500 | 600 | 700 | 800;
        style: "italic" | "normal";
        axis: "MONO";
    };
    S78: {
        weight: 1 | 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900 | 1000;
        style: "normal";
        axis: "ROND" | "opsz" | "slnt" | "wdth";
    };
    S79: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "normal";
        axis: "ELGR" | "ELSH";
    };
    S8: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700;
        style: "normal";
        axis: never;
    };
    S80: {
        weight: 1 | 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "normal";
        axis: never;
    };
    S81: {
        weight: 400;
        style: "normal";
        axis: "MORF" | "SHLN";
    };
    S82: {
        weight: 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "italic" | "normal";
        axis: "wdth";
    };
    S83: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700;
        style: "italic" | "normal";
        axis: "wdth";
    };
    S84: {
        weight: 200 | 300 | 400;
        style: "normal";
        axis: never;
    };
    S85: {
        weight: 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "normal";
        axis: "wdth";
    };
    S86: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "italic" | "normal";
        axis: "opsz";
    };
    S87: {
        weight: 300 | 400 | 500 | 700;
        style: "italic" | "normal";
        axis: never;
    };
    S88: {
        weight: 400 | 500 | 700 | 800;
        style: "normal";
        axis: never;
    };
    S89: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700;
        style: "normal";
        axis: "wdth";
    };
    S9: {
        weight: 400;
        style: "normal";
        axis: "MORF";
    };
    S90: {
        weight: 300 | 400 | 600 | 700 | 800;
        style: "normal";
        axis: never;
    };
    S91: {
        weight: 300 | 400 | 500;
        style: "normal";
        axis: never;
    };
    S92: {
        weight: 400 | 600;
        style: "normal";
        axis: never;
    };
    S93: {
        weight: 400 | 500 | 600 | 700;
        style: "normal";
        axis: "ROND" | "wdth";
    };
    S94: {
        weight: 200 | 300 | 400 | 600 | 700;
        style: "italic" | "normal";
        axis: never;
    };
    S95: {
        weight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
        style: "normal";
        axis: "YOPQ";
    };
    S96: {
        weight: 100 | 400 | 700 | 800;
        style: "normal";
        axis: never;
    };
    S97: {
        weight: 100 | 300 | 400 | 700 | 900;
        style: "italic" | "normal";
        axis: never;
    };
    S98: {
        weight: 400;
        style: "normal";
        axis: "wdth";
    };
    S99: {
        weight: 400 | 700 | 800;
        style: "normal";
        axis: never;
    };
}

export type GoogleFontsOptions = FetchOptions & {
    families: GoogleFontFamily[];
    display?: "auto" | "block" | "swap" | "fallback" | "optional";
    cache?: Pick<Map<string, Promise<string>>, "get" | "set" | "delete">;
    baseUrl?: string;
};

export function image(props: Omit<ImageNode, "type">): ImageNode;

export interface ImageFetchCache {
    delete(url: string): unknown;
    get(url: string): Promise<ArrayBuffer> | undefined;
    set(url: string, data: Promise<ArrayBuffer>): unknown;
}

export type ImageNode = NodeMetadata & {
    type: "image";
    src: string | Uint8Array | ArrayBuffer | RgbaImage;
    width?: number;
    height?: number;
};

// Warning: (ae-forgotten-export) The symbol "GoogleFontShapes" needs to be exported
type KnownGoogleFontFamily = { [S in keyof GoogleFontShapes]: {
        name: GoogleFontShapeFamilies[S];
        weight?: GoogleFontShapes[S]["weight"] | GoogleFontShapes[S]["weight"][] | WeightRange;
        style?: GoogleFontShapes[S]["style"] | GoogleFontShapes[S]["style"][];
        axes?: [GoogleFontShapes[S]["axis"]] extends [never] ? never : Partial<Record<GoogleFontShapes[S]["axis"], AxisValue>>;
        generic?: GenericFontFamily;
    }; }[keyof GoogleFontShapes];

export type LayerRule = {
    layer: string;
    rules?: CssInput[];
};

// @deprecated
export const LIST_MARKER_CHARACTERS = "•◦■▪▸◂▾ 0123456789.-abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

export type MediaRule = {
    media: string;
    rules: CssInput[];
};

type Node_2 = ContainerNode | TextNode | ImageNode;
export { Node_2 as Node }

export type NodeAttributes = Record<string, string>;

export type NodeMetadata = {
    tagName?: string;
    className?: string;
    id?: string;
    dir?: "ltr" | "rtl";
    lang?: string;
    attributes?: NodeAttributes;
    tw?: string;
    style?: Declarations;
    preset?: Declarations;
};

export function percentage(percentage: number): `${number}%`;

// @deprecated
export function prepareImages<T extends {
    src: string;
} = FetchedImage>(input: PrepareImagesOptions<T>): Promise<(T | FetchedImage)[]>;

export type PrepareImagesOptions<T extends {
    src: string;
} = FetchedImage> = FetchOptions & {
    urls?: Iterable<string>;
    node?: Node_2 | Node_2[];
    sources?: T[];
    fetchCache?: ImageFetchCache;
    throwOnError?: boolean;
};

export type ReactElementLike = {
    type: string | symbol | ((props: never) => unknown) | (new (props: never) => unknown) | ReactElementLike;
    props: unknown;
    $$typeof?: symbol | string;
};

export function readBodyLimited(response: Response, maxBytes: number): Promise<ArrayBuffer>;

export function rem(rem: number): `${number}rem`;

export function rgba(r: number, g: number, b: number, a?: number): `rgb(${number} ${number} ${number} / ${number})`;

export type RgbaImage = {
    width: number;
    height: number;
    data: Uint8Array | ArrayBuffer;
    premultiplied?: boolean;
};

export function style(style: Declarations): CSSProperties;

export type StyleRule = {
    selector: string;
    style?: Declarations;
    rules?: StyleRule[];
};

// @deprecated
export function subsetFonts<T>(input: {
    fonts: T[];
    source: CodepointSource;
}): T[];

export type SupportsRule = {
    supports: string;
    rules: CssInput[];
};

export function text(text: string, style?: Declarations): TextNode;

export function text(props: Omit<TextNode, "type">): TextNode;

export type TextFit = "none" | "grow" | "shrink" | "consistent" | "per-line" | "per-line-all" | `${number}%` | (string & {});

export type TextNode = NodeMetadata & {
    type: "text";
    text: string;
};

export function vh(vh: number): `${number}vh`;

export function vw(vw: number): `${number}vw`;

type WeightRange = `${number}..${number}`;

// Warnings were encountered during analysis:
// dist/fonts-DYcYugsN.d.mts:982:3 - (ae-forgotten-export) The symbol "FontStyle" needs to be exported
// dist/fonts-DYcYugsN.d.mts:983:3 - (ae-forgotten-export) The symbol "AxisValue" needs to be exported
```

## `@takumi-rs/helpers/html`

```ts
import { CSSProperties } from 'react';

// Warning: (ae-forgotten-export) The symbol "NodeMetadata" needs to be exported
type ContainerNode = NodeMetadata & {
    type: "container";
    children?: Node_2[];
};

type Declarations = CSSProperties;

// @deprecated
export function fromHtml(html: string): FromHtmlResult;

// Warning: (ae-forgotten-export) The symbol "FromJsxResult" needs to be exported
export interface FromHtmlResult extends FromJsxResult {}

interface FromJsxResult {
    css: string[];
    node: Node_2;
    // @deprecated
    stylesheets: string[];
}

type ImageNode = NodeMetadata & {
    type: "image";
    src: string | Uint8Array | ArrayBuffer | RgbaImage;
    width?: number;
    height?: number;
};

// Warning: (ae-forgotten-export) The symbol "ContainerNode" needs to be exported
// Warning: (ae-forgotten-export) The symbol "TextNode" needs to be exported
// Warning: (ae-forgotten-export) The symbol "ImageNode" needs to be exported
type Node_2 = ContainerNode | TextNode | ImageNode;

type NodeAttributes = Record<string, string>;

type NodeMetadata = {
    tagName?: string;
    className?: string;
    id?: string;
    dir?: "ltr" | "rtl";
    lang?: string;
    attributes?: NodeAttributes;
    tw?: string;
    style?: Declarations;
    preset?: Declarations;
};

type RgbaImage = {
    width: number;
    height: number;
    data: Uint8Array | ArrayBuffer;
    premultiplied?: boolean;
};

type TextNode = NodeMetadata & {
    type: "text";
    text: string;
};

// Warnings were encountered during analysis:
// dist/types-CtSMAucZ.d.mts:60:3 - (ae-forgotten-export) The symbol "NodeAttributes" needs to be exported
// dist/types-CtSMAucZ.d.mts:62:3 - (ae-forgotten-export) The symbol "Declarations" needs to be exported
// dist/types-CtSMAucZ.d.mts:68:3 - (ae-forgotten-export) The symbol "Node_2" needs to be exported
// dist/types-CtSMAucZ.d.mts:87:3 - (ae-forgotten-export) The symbol "RgbaImage" needs to be exported
```

## `@takumi-rs/helpers/jsx`

```ts
import { ComponentProps } from 'react';
import { CSSProperties } from 'react';
import { JSX } from 'react';
import { ReactElement } from 'react';
import { ReactNode } from 'react';

// Warning: (ae-forgotten-export) The symbol "BitmapImgProps" needs to be exported
export function Bitmap(input: BitmapProps): ReactElement<BitmapImgProps, "img">;

// Warning: (ae-forgotten-export) The symbol "RgbaImage" needs to be exported
type BitmapImgProps = Omit<BitmapProps, keyof RgbaImage> & {
    src: RgbaImage;
};

export type BitmapProps = RgbaImage & Omit<ComponentProps<"img">, "src" | "width" | "height">;

// Warning: (ae-forgotten-export) The symbol "NodeMetadata" needs to be exported
type ContainerNode = NodeMetadata & {
    type: "container";
    children?: Node_2[];
};

type Declarations = CSSProperties;

export const defaultStylePresets: Partial<Record<keyof JSX.IntrinsicElements, CSSProperties>>;

// Warning: (ae-forgotten-export) The symbol "ReactElementLike" needs to be exported
export function fromJsx(element: ReactNode | ReactElementLike, options?: FromJsxOptions): Promise<FromJsxResult>;

export interface FromJsxOptions {
    defaultStyles?: typeof defaultStylePresets | false;
    tailwindClassesProperty?: string;
}

export interface FromJsxResult {
    css: string[];
    node: Node_2;
    // @deprecated
    stylesheets: string[];
}

export type HtmlProps = {
    className?: string;
    class?: string;
    id?: string;
    style?: string | Declarations;
    dir?: string;
    lang?: string;
    [key: string]: unknown;
};

type ImageNode = NodeMetadata & {
    type: "image";
    src: string | Uint8Array | ArrayBuffer | RgbaImage;
    width?: number;
    height?: number;
};

// Warning: (ae-forgotten-export) The symbol "ContainerNode" needs to be exported
// Warning: (ae-forgotten-export) The symbol "TextNode" needs to be exported
// Warning: (ae-forgotten-export) The symbol "ImageNode" needs to be exported
type Node_2 = ContainerNode | TextNode | ImageNode;

type NodeAttributes = Record<string, string>;

type NodeMetadata = {
    tagName?: string;
    className?: string;
    id?: string;
    dir?: "ltr" | "rtl";
    lang?: string;
    attributes?: NodeAttributes;
    tw?: string;
    style?: Declarations;
    preset?: Declarations;
};

type ReactElementLike = {
    type: string | symbol | ((props: never) => unknown) | (new (props: never) => unknown) | ReactElementLike;
    props: unknown;
    $$typeof?: symbol | string;
};

type RgbaImage = {
    width: number;
    height: number;
    data: Uint8Array | ArrayBuffer;
    premultiplied?: boolean;
};

type TextNode = NodeMetadata & {
    type: "text";
    text: string;
};

// Warnings were encountered during analysis:
// dist/index-MezLWWhL.d.mts:11:3 - (ae-forgotten-export) The symbol "Declarations" needs to be exported
// dist/types-CtSMAucZ.d.mts:60:3 - (ae-forgotten-export) The symbol "NodeAttributes" needs to be exported
// dist/types-CtSMAucZ.d.mts:68:3 - (ae-forgotten-export) The symbol "Node_2" needs to be exported
```

## `@takumi-rs/helpers/emoji`

```ts
import { CSSProperties } from 'react';

// Warning: (ae-forgotten-export) The symbol "NodeMetadata" needs to be exported
type ContainerNode = NodeMetadata & {
    type: "container";
    children?: Node_2[];
};

type Declarations = CSSProperties;

export type EmojiType = "twemoji" | "blobmoji" | "noto" | "openmoji" | "fluent" | "fluentFlat";

export function extractEmojis(node: Node_2, emojiType: EmojiType): Node_2;

type ImageNode = NodeMetadata & {
    type: "image";
    src: string | Uint8Array | ArrayBuffer | RgbaImage;
    width?: number;
    height?: number;
};

export function isEmojiImageUrl(url: string): boolean;

// Warning: (ae-forgotten-export) The symbol "ContainerNode" needs to be exported
// Warning: (ae-forgotten-export) The symbol "TextNode" needs to be exported
// Warning: (ae-forgotten-export) The symbol "ImageNode" needs to be exported
type Node_2 = ContainerNode | TextNode | ImageNode;

type NodeAttributes = Record<string, string>;

type NodeMetadata = {
    tagName?: string;
    className?: string;
    id?: string;
    dir?: "ltr" | "rtl";
    lang?: string;
    attributes?: NodeAttributes;
    tw?: string;
    style?: Declarations;
    preset?: Declarations;
};

type RgbaImage = {
    width: number;
    height: number;
    data: Uint8Array | ArrayBuffer;
    premultiplied?: boolean;
};

type TextNode = NodeMetadata & {
    type: "text";
    text: string;
};

// Warnings were encountered during analysis:
// dist/types-CtSMAucZ.d.mts:60:3 - (ae-forgotten-export) The symbol "NodeAttributes" needs to be exported
// dist/types-CtSMAucZ.d.mts:62:3 - (ae-forgotten-export) The symbol "Declarations" needs to be exported
// dist/types-CtSMAucZ.d.mts:68:3 - (ae-forgotten-export) The symbol "Node_2" needs to be exported
// dist/types-CtSMAucZ.d.mts:87:3 - (ae-forgotten-export) The symbol "RgbaImage" needs to be exported
```

## `@takumi-rs/helpers/renderer`

```ts
import { CSSProperties } from 'react';

// @internal @deprecated
export type AnimationOutputFormatOptions = {
    format?: "webp";
    quality?: number;
    lossless?: boolean;
} | {
    format: "apng";
} | {
    format: "gif";
};

type Awaitable<T> = T | Promise<T>;

// @internal @deprecated
export type BackendAnimationOptions<TInternal> = Omit<TInternal, "images" | "format" | "quality" | "lossless"> & AnimationOutputFormatOptions & RenderExtras;

// @internal @deprecated
export type BackendRenderOptions<TInternal> = Omit<TInternal, "images" | "format" | "quality" | "lossless"> & OutputFormatOptions & RenderExtras;

// @internal @deprecated
export type BackendSvgOptions<TInternal> = Omit<TInternal, "images"> & RenderExtras;

// @internal @deprecated
export type ByteBuf = Uint8Array | ArrayBuffer | Buffer;

// Warning: (ae-forgotten-export) The symbol "Node_2" needs to be exported
type CodepointSource = string | Node_2 | (string | Node_2)[];

// Warning: (ae-forgotten-export) The symbol "NodeMetadata" needs to be exported
type ContainerNode = NodeMetadata & {
    type: "container";
    children?: Node_2[];
};

type Declarations = CSSProperties;

// @internal @deprecated
export type Font = FontDetails | ByteBuf;

// @internal @deprecated
export type FontBackend<TFamily extends RegisteredFamilyLike> = {
    registerFont(font: Font, key: string): Awaitable<TFamily[]>;
    cacheFont(font: Font, key: string): Awaitable<TFamily[]>;
    cachedFonts(key: string, pin: boolean): TFamily[] | null | undefined;
};

// @internal @deprecated
export type FontDetails = {
    name?: string;
    data: ByteBuf;
    weight?: number;
    style?: "normal" | "italic" | "oblique" | `oblique ${number}deg` | (string & {});
    unicodeRange?: [number, number][];
    subsetOf?: string;
    subsetRank?: number;
    generic?: GenericFontFamily;
};

// @internal @deprecated
export type FontLoader = string | Font | (Omit<FontDetails, "data"> & {
    data: () => Promise<FontDetails["data"]> | FontDetails["data"];
    ranges?: [number, number][];
} & ({
    key: string;
} | {
    name: string;
}));

// @internal @deprecated
export class FontRegistry<TFamily extends RegisteredFamilyLike> {
    constructor(backend: FontBackend<TFamily>);
    register(font: FontLoader, pinned?: boolean): Promise<TFamily[]>;
    resolveResources<T>(fonts: FontLoader[] | undefined, images: ImagesInput | undefined, fontFamilies: string[] | undefined, use: (resources: {
        images?: ImageSource[];
        fontFamilies?: string[];
    }) => Awaitable<T>): Promise<T>;
}

type GenericFontFamily = "serif" | "sans-serif" | "monospace" | "cursive" | "fantasy" | "system-ui" | "ui-serif" | "ui-sans-serif" | "ui-monospace" | "ui-rounded" | "emoji" | "math" | "fangsong";

// @internal @deprecated
export type ImageCacheMode = "auto" | "none";

// @internal @deprecated
export type ImageLoader = Omit<ImageSource, "data"> & {
    data: ImageSource["data"] | (() => ImageSource["data"] | Promise<ImageSource["data"]>);
};

type ImageNode = NodeMetadata & {
    type: "image";
    src: string | Uint8Array | ArrayBuffer | RgbaImage;
    width?: number;
    height?: number;
};

// @internal @deprecated
export type ImagesInput = ImageLoader[] | {
    sources?: ImageLoader[];
    cache?: NonNullable<ImageLoader["cache"]>;
};

// @internal @deprecated
export type ImageSource = {
    src: string;
    data: ByteBuf;
    cache?: ImageCacheMode;
};

// @internal @deprecated
export type JsonSource = {
    json: string;
};

// Warning: (ae-forgotten-export) The symbol "ContainerNode" needs to be exported
// Warning: (ae-forgotten-export) The symbol "TextNode" needs to be exported
// Warning: (ae-forgotten-export) The symbol "ImageNode" needs to be exported
type Node_2 = ContainerNode | TextNode | ImageNode;

type NodeAttributes = Record<string, string>;

type NodeMetadata = {
    tagName?: string;
    className?: string;
    id?: string;
    dir?: "ltr" | "rtl";
    lang?: string;
    attributes?: NodeAttributes;
    tw?: string;
    style?: Declarations;
    preset?: Declarations;
};

// @internal @deprecated
export type OutputFormatOptions = {
    format?: "png";
} | {
    format: "jpeg";
    quality?: number;
} | {
    format: "webp";
    quality?: number;
    lossless?: boolean;
} | {
    format: "ico";
} | {
    format: "raw";
};

// @internal @deprecated
export type RegisteredFamilyLike = {
    name: string;
};

// @internal @deprecated
export type RenderExtras = {
    fonts?: FontLoader[] | Promise<FontLoader[]>;
    signal?: AbortSignal;
    images?: ImagesInput;
};

// @internal @deprecated
export type ResolvedRenderOptions<TOptions> = Omit<TOptions, keyof RenderExtras | "fontFamilies"> & {
    images?: ImageSource[];
    fontFamilies?: string[];
};

// @internal @deprecated
export function resolveImageLoaders(images: ImagesInput): Promise<ImageSource[]>;

type RgbaImage = {
    width: number;
    height: number;
    data: Uint8Array | ArrayBuffer;
    premultiplied?: boolean;
};

type TextNode = NodeMetadata & {
    type: "text";
    text: string;
};

// @internal @deprecated
export function toNativeSource(source: Node_2 | string): Node_2 | string | JsonSource;

// Warning: (ae-forgotten-export) The symbol "CodepointSource" needs to be exported
// @internal @deprecated
export function withRenderInput<TOptions extends RenderExtras & {
    fontFamilies?: string[];
}, TFamily extends RegisteredFamilyLike, TResult>(registry: FontRegistry<TFamily>, options: TOptions, source: CodepointSource, call: (options: ResolvedRenderOptions<TOptions>, signal: AbortSignal | undefined) => Awaitable<TResult>): Promise<TResult>;

// Warnings were encountered during analysis:
// dist/renderer.d.mts:63:3 - (ae-forgotten-export) The symbol "GenericFontFamily" needs to be exported
// dist/renderer.d.mts:222:3 - (ae-forgotten-export) The symbol "Awaitable" needs to be exported
// dist/types-CtSMAucZ.d.mts:60:3 - (ae-forgotten-export) The symbol "NodeAttributes" needs to be exported
// dist/types-CtSMAucZ.d.mts:62:3 - (ae-forgotten-export) The symbol "Declarations" needs to be exported
// dist/types-CtSMAucZ.d.mts:87:3 - (ae-forgotten-export) The symbol "RgbaImage" needs to be exported
```
