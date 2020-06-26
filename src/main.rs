// Example 2: The Image
// Draw an image to the screen
use quicksilver::{
    geom::{Rectangle, Vector},
    graphics::{Color, Image},
    run, Graphics, Input, Result, Settings, Window,
};

use std::collections::HashMap;

#[macro_use]
extern crate log;

mod assets;

fn main() {
    run(
        Settings {
            size: Vector::new(2048.0, 1024.0),
            title: "Maverick",
            log_level: log::Level::Info,
            ..Settings::default()
        },
        app,
    );
}

/// Special abilities that some monsters have
#[derive(Debug, Copy, Clone)]
enum Ability {
    Noxious,
    Rally,
    Reign
}

/// Actions needed to be performed on a monster in order to kill it
#[derive(Debug, Copy, Clone)]
enum ToSlay {
    Melee,
    Range,
    Move
}

/// Monster stats
struct Monsters {
    images: Vec<Image>,
    names: Vec<&'static str>,
    strengths: Vec<u8>,
    abilities: Vec<Option<Ability>>,
    to_slays: Vec<Vec<ToSlay>>
}

type MonsterStats = (&'static str, u8, Option<Ability>, [Option<ToSlay>; 3]);

const MONSTER_DECK_SIZE: usize = 13;

/// Monster stats for the available monsters
const MONSTER_STATS: [MonsterStats; 18] = [
    ("Banshee",    1, None, [Some(ToSlay::Melee), Some(ToSlay::Range), None]),
    ("Beholder",   1, None, [Some(ToSlay::Range), Some(ToSlay::Range), Some(ToSlay::Range)]),
    ("Bug",        1, None, [Some(ToSlay::Range), None, None]),
    ("Demon",      5, None, [Some(ToSlay::Melee), Some(ToSlay::Range), None]),
    ("Dragon",     5, Some(Ability::Reign), [Some(ToSlay::Move), Some(ToSlay::Melee), Some(ToSlay::Range)]),
    ("Elemental",  2, None, [Some(ToSlay::Melee), Some(ToSlay::Range), None]),
    ("Ghost",      0, Some(Ability::Noxious), [Some(ToSlay::Melee), None, None]),
    ("Golem",      3, Some(Ability::Reign), [Some(ToSlay::Melee), None, None]),
    ("Hellhound",  2, None, [Some(ToSlay::Range), None, None]),
    ("Howler",     4, Some(Ability::Rally), [Some(ToSlay::Melee), None, None]),
    ("Imp",        0, None, [Some(ToSlay::Move), Some(ToSlay::Move), None]),
    ("Lich",       4, Some(Ability::Reign), [Some(ToSlay::Range), None, None]),
    ("Scorpion",   1, Some(Ability::Noxious), [Some(ToSlay::Melee), Some(ToSlay::Range), None]),
    ("Skeleton",   2, None, [Some(ToSlay::Move), Some(ToSlay::Melee), None]),
    ("Spider",     1, Some(Ability::Noxious), [Some(ToSlay::Range), Some(ToSlay::Range), None]),
    ("Troglodyte", 1, Some(Ability::Rally), [Some(ToSlay::Move), Some(ToSlay::Melee), None]),
    ("Troll",      3, None, [Some(ToSlay::Move), Some(ToSlay::Range), None]),
    ("Werewolf",   2, None, [Some(ToSlay::Melee), Some(ToSlay::Melee), None]),
];

impl Monsters {
    /// Initialize the monster deck for this game. `Graphics` is needed to create 
    /// the image for each monster.
    pub async fn init(gfx: &Graphics) -> Result<Monsters> {
        // Create the monster deck via a random selection of 13 monsters
        let mut monster_indexes = Vec::new();
        loop {
            // If we h
            if monster_indexes.len() == MONSTER_DECK_SIZE {
                break;
            }

            let mut index = rand::random::<usize>() % MONSTER_STATS.len();
            loop {
                if !monster_indexes.contains(&index) {
                    monster_indexes.push(index);
                    break;
                }

                index = rand::random::<usize>() % MONSTER_STATS.len();
            }
        }

        // Init the monsters struct
        let mut monsters = Monsters {
            images: Vec::new(),
            names: Vec::new(),
            strengths: Vec::new(),
            abilities: Vec::new(),
            to_slays: Vec::new(),
        };

        // Populate the Monsters struct
        for &index in &monster_indexes {
            // Get the monster stats for the current monster
            let (name, strength, ability, to_slay) = MONSTER_STATS[index];

            // Get the monster image
            info!("Getting image: {}", name);
            let image = Image::load(&gfx, format!("monsters_small/{}.png", name)).await?;
            monsters.images.push(image);

            // Populate these monster fields
            monsters.names.push(name);
            monsters.strengths.push(strength);
            monsters.abilities.push(ability);

            // Create a Vec from only the valid ToSlay
            let curr_slay: Vec<ToSlay> = to_slay.iter()
                                                .filter(|x| x.is_some())
                                                .map(|x| x.unwrap())
                                                .collect();

            // Add the allocated vec to the Monsters
            monsters.to_slays.push(curr_slay);
        }

        Ok(monsters)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
/// Card types for keys of the images loaded
enum CardType {
    RegPlayer,
    BigPlayer,
    MeleeCompanion,
    RangeCompanion,
    Action(u8)
}

/// Current player kind. Player starts as `Regular` and shifts to `Big` if 5 actions are
/// spent on any one turn
enum PlayerKind {
    Regular,
    Big
}

/// Types of companions available
enum CompanionKind {
    Melee,
    Range
}

/// Global struct for handling Game State
struct Game {
    /// Monsters in this game
    monsters: Monsters,

    /// Current index of the player
    player_index: usize,

    /// Type of player currently active
    player_kind: PlayerKind,

    /// Current index of the companion
    companion_index: usize,

    /// Type of companion currently active
    companion_kind: CompanionKind,

    /// Image cache
    images: HashMap<CardType, Image>
}

impl Game {
    pub async fn init(gfx: &Graphics) -> Result<Game> {
        // Create the monster deck for this game
        let monsters = Monsters::init(&gfx).await?;

        let mut images = HashMap::new();
        images.insert(CardType::RegPlayer, Image::load(&gfx, format!("characters_small/main.png")).await?);
        images.insert(CardType::BigPlayer, Image::load(&gfx, format!("characters_small/big.jpg")).await?);
        images.insert(CardType::MeleeCompanion, Image::load(&gfx, format!("companions_small/melee.png")).await?);
        images.insert(CardType::RangeCompanion, Image::load(&gfx, format!("companions_small/range.png")).await?);
        images.insert(CardType::Action(1), Image::load(&gfx, format!("actions_small/1.png")).await?);
        images.insert(CardType::Action(2), Image::load(&gfx, format!("actions_small/2.png")).await?);
        images.insert(CardType::Action(3), Image::load(&gfx, format!("actions_small/3.png")).await?);
        images.insert(CardType::Action(4), Image::load(&gfx, format!("actions_small/4.png")).await?);
        images.insert(CardType::Action(5), Image::load(&gfx, format!("actions_small/5.png")).await?);

        let companion_kind = match rand::random::<u8>() & 1 {
            0 => CompanionKind::Melee,
            1 => CompanionKind::Range,
            _ => unreachable!()
        };

        Ok(Game {
            monsters,
            player_index: 6,
            companion_index: 4,
            images,
            player_kind: PlayerKind::Regular,
            companion_kind
        })
    }

    /// Draw the current game state using the given `Graphics`
    pub async fn draw(&self, window: &Window, gfx: &mut Graphics) -> Result<()> {
        const PADDING: f32 = 10.0;

        // Start row 1 from `PADDING` from the top
        let mut curr_y = PADDING;

        /* Row 1 */
        // Get the card type for the current player
        let image = match self.player_kind {
            PlayerKind::Regular => &self.images[&CardType::RegPlayer],
            PlayerKind::Big =>     &self.images[&CardType::BigPlayer],
        };

        // Calculate the X coord based on the player index
        let image_width = image.size().x;
        let curr_x = PADDING + (image_width + PADDING) * self.player_index as f32;

        // Draw the player image in Row 1
        let region = Rectangle::new(Vector::new(curr_x, curr_y), image.size());
        gfx.draw_image(&image, region);
        /* End Row 1 */

        // Adjust the row to the second row
        curr_y += image.size().y + PADDING;

        /* Row 2 */
        let mut curr_x = PADDING;
        for monster_index in 0..MONSTER_DECK_SIZE {
            let image = &self.monsters.images[monster_index];
            let image_size = image.size();

            // Draw each image in Row 2
            let region = Rectangle::new(Vector::new(curr_x, curr_y), image_size);
            gfx.draw_image(&image, region);

            let width = image_size.x;
            curr_x += PADDING + width;
        }
        /* End Row 2 */

        // Adjust the row to the third row
        curr_y += image.size().y + PADDING;

        /* Row 3 */
        let image = match self.companion_kind {
            CompanionKind::Melee => &self.images[&CardType::MeleeCompanion],
            CompanionKind::Range => &self.images[&CardType::RangeCompanion],
        };

        // Calculate the X coord based on the player index
        let image_width = image.size().x;
        let curr_x = PADDING + (image_width + PADDING) * self.companion_index as f32;

        // Draw the player image in Row 1
        let region = Rectangle::new(Vector::new(curr_x, curr_y), image.size());
        gfx.draw_image(&image, region);
        /* End Row 3 */

        gfx.present(&window)
    }
}

// This time we might return an error, so we use a Result
async fn app(window: Window, mut gfx: Graphics, mut input: Input) -> Result<()> {
    // Load the image and wait for it to finish
    // We also use '?' to handle errors like file-not-found
    gfx.clear(Color::BLACK);

    let game = Game::init(&gfx).await?;
    game.draw(&window, &mut gfx).await?;

    loop {
        while let Some(_) = input.next_event().await {}
    }
}

