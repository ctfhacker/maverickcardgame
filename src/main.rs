#![feature(exclusive_range_pattern)]
#![feature(vec_remove_item)]

use quicksilver::{
    geom::{Rectangle, Vector},
    geom::Shape,
    graphics::{Color, Image, VectorFont},
    input::*,
    run, Graphics, Input, Result, Settings, Window,
};

use std::collections::HashMap;

#[macro_use]
extern crate log;

mod assets;

mod consts;
use consts::*;

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

/// Which entity an action can be performed on.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Entity {
    Character,
    Companion
}

/// Direction which an ability is performed in the dungeon row
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Direction {
    Left,
    Right
}

/// Available actions the player can perform in the game
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Action {
    Range(Entity, Direction),
    Melee(Entity),
    Move(Entity, Direction),
    Swap,
    EndTurn
}

/// Special abilities that some monsters have
#[derive(Debug, Copy, Clone)]
enum Ability {
    Noxious,
    Rally,
    Reign
}

/// Actions needed to be performed on a monster in order to kill it
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
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
    strength_adjustments: Vec<u8>,
    abilities: Vec<Option<Ability>>,
    to_slays: Vec<Vec<ToSlay>>,
    current_hits: Vec<Vec<ToSlay>>,
    alive: Vec<bool>
}

type MonsterStats = (&'static str, u8, Option<Ability>, [Option<ToSlay>; 3]);


/// Monster stats for the available monsters
const MONSTER_STATS: [MonsterStats; 18] = [
    ("Banshee",    1, None, [Some(ToSlay::Melee), Some(ToSlay::Range), None]),
    ("Beholder",   1, None, [Some(ToSlay::Range), Some(ToSlay::Range), Some(ToSlay::Range)]),
    ("Bug",        1, None, [Some(ToSlay::Range), None, None]),
    ("Demon",      5, None, [Some(ToSlay::Melee), Some(ToSlay::Range), None]),
    ("Dragon",     5, Some(Ability::Reign), [Some(ToSlay::Move), Some(ToSlay::Melee), Some(ToSlay::Range)]),
    ("Elemental",  2, None, [Some(ToSlay::Melee), Some(ToSlay::Range), None]),
    ("Ghost",      0, Some(Ability::Noxious), [Some(ToSlay::Move), None, None]),
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
            strength_adjustments: Vec::new(),
            abilities: Vec::new(),
            to_slays: Vec::new(),
            current_hits: Vec::new(),
            alive: Vec::new(),
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
            monsters.strength_adjustments.push(0);
            monsters.abilities.push(ability);
            monsters.alive.push(true);

            // Create a Vec from only the valid ToSlay
            let curr_slay: Vec<ToSlay> = to_slay.iter()
                                                .filter(|x| x.is_some())
                                                .map(|x| x.unwrap())
                                                .collect();

            // Add the allocated vec to the Monsters
            monsters.to_slays.push(curr_slay);

            // Init the current hits for each monster 
            monsters.current_hits.push(Vec::new());
        }

        Ok(monsters)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
/// Asset types for keys of the images loaded
enum AssetType {
    RegPlayer,
    MonstrousPlayer,
    MeleeCompanion,
    RangeCompanion,
    Action(u8),
    MeleeTarget,
    RangeTarget,
    MoveTarget,
    SwapTarget,
    ReignTarget,
    CardBack
}

/// Current player kind. Player starts as `Regular` and shifts to `Monstrous` if 5 actions are
/// spent on any one turn
enum PlayerKind {
    Regular,
    Monstrous
}

/// Types of companions available
enum CompanionKind {
    Melee,
    Range
}

/// States of the game itself
#[derive(Debug, Copy, Clone)]
enum State {
    Playing,
    EndGame,
    Reset
}

/// Type of action resulting from a click
#[derive(Debug, Copy, Clone)]
enum ClickableType {
    Action(Action),
    Card(usize),
    State(State)
}

/// Global struct for handling Game State
struct Game {
    /// Current game state of the game
    state: State,

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
    images: HashMap<AssetType, Image>,

    /// Font used for text in the game
    font: VectorFont,

    /// Clickable regions in the current board state. This cache is updated every `draw()` call
    /// to query whether a mouse click should trigger an action
    clickables: Vec<(Rectangle, ClickableType)>,

    /// Deck containing the action cards
    deck: Vec<u8>,

    /// Current hand
    hand: Vec<u8>,

    /// Maximum hand size: 5 for regular player and 6 when player becomes Monstrous by playing
    /// 5 actions in one turn
    hand_limit: u8,

    /// Currently selected action
    current_action: Option<Action>,

    /// Index of the card currently selected
    current_card: Option<usize>,

    /// Has a card been discarded in a turn. Used when checking if to transform to Monstrous form
    discarded: bool,

    /// Number of cards withheld at the beginning of the game. Worth 3 points each if game is won.
    payments: u32,

    /// Trophies gathered during the course of the game
    trophies: u32,
}

impl Game {
    pub async fn init(gfx: &Graphics) -> Result<Game> {
        // Create the monster deck for this game
        let monsters = Monsters::init(&gfx).await?;

        let mut images = HashMap::new();

        for (asset_type, path) in [
            (AssetType::RegPlayer, "characters_small/main_crop.png"),
            (AssetType::MonstrousPlayer, "characters_small/big_crop.png"),
            (AssetType::MeleeCompanion, "companions_small/melee_crop.png"),
            (AssetType::RangeCompanion, "companions_small/range_crop.png"),
            (AssetType::Action(1),"actions_small/1black.png"),
            (AssetType::Action(2),"actions_small/2black.png"),
            (AssetType::Action(3),"actions_small/3black.png"),
            (AssetType::Action(4),"actions_small/4black.png"),
            (AssetType::Action(5),"actions_small/5black.png"),
            (AssetType::MeleeTarget,"targets/melee.png"),
            (AssetType::RangeTarget,"targets/range.png"),
            (AssetType::MoveTarget,"targets/move.png"),
            (AssetType::SwapTarget,"targets/swap.png"),
            (AssetType::ReignTarget,"targets/reign.png"),
            (AssetType::CardBack, "action.png"),
        ].iter() {
            images.insert(*asset_type, Image::load(&gfx, &path).await?);
        }


        let companion_kind = match rand::random::<u8>() & 1 {
            0 => CompanionKind::Melee,
            1 => CompanionKind::Range,
            _ => unreachable!()
        };

        // Generate the deck itself
        let mut deck = vec![
            1, 1, 1, 1, 1, 1, 1, 1,  
            2, 2, 2, 2, 2, 2, 2, 2,  
            3, 3, 3, 3, 3, 3, 3, 3,  
            4, 4, 4, 4, 4, 4, 4, 4,  
            5, 5, 5, 5, 5, 5, 5, 5
        ];

        // Number of initial cards removed 
        let payments = 1;

        // Shuffle the deck
        for _ in 0..1000 {
            let x = rand::random::<usize>() % deck.len();
            let y = rand::random::<usize>() % deck.len();
            if x == y {
                continue;
            }
            deck.swap(x, y);
        }

        // Discard cards equal to payment
        for _ in 0..payments { deck.pop(); }

        // Populate the initial hand
        let mut hand = Vec::new();
        for _ in 0..5 {
            hand.push(deck.pop().unwrap());
        }

        Ok(Game {
            state: State::Playing,
            monsters,
            player_index: 0,
            companion_index: 0,
            images,
            player_kind: PlayerKind::Regular,
            companion_kind,
            clickables: Vec::new(),
            font: VectorFont::load("iosevka-regular.ttf").await?,
            deck,
            hand,
            hand_limit: 5,
            current_action: None,
            current_card: None,
            discarded: false,
            payments,
            trophies: 0
        })
    }

    /// Draw the current game state using the given `Graphics`
    pub async fn draw(&mut self, window: &Window, mut gfx: &mut Graphics) 
            -> Result<()> {
        if matches!(self.state, State::EndGame) {
            gfx.clear(Color::BLACK);
            let mut font = self.font.to_renderer(&gfx, 48.0)?;

            font.draw( 
                &mut gfx,
                "Game over!",
                Color::RED,
                Vector::new(10.0, 100.0),
            )?;

            let message = if self.monsters.alive.iter().any(|&x| x == true) {
                "YOU LOST!"
            } else {
                "YOU WON!"
            };

            font.draw( 
                &mut gfx,
                message,
                Color::RED,
                Vector::new(10.0, 150.0),
            )?;

            font.draw( 
                &mut gfx,
                "Click to reset..",
                Color::RED,
                Vector::new(10.0, 200.0),
            )?;

            self.clickables.clear();
            let fullscreen = Rectangle::new(Vector::new(5.0, 160.0), Vector::new(350.0, 50.0));
            self.clickables.push((fullscreen, ClickableType::State(State::Reset)));

            font.draw( 
                &mut gfx,
                "Score:",
                Color::RED,
                Vector::new(10.0, 300.0),
            )?;

            font.draw( 
                &mut gfx,
                &format!("Payments:   {} ({} * 3)", self.payments * 3, self.payments),
                Color::RED,
                Vector::new(10.0, 350.0),
            )?;

            font.draw( 
                &mut gfx,
                &format!("Trophies:   {} ({} * 2)", self.trophies * 2, self.trophies),
                Color::RED,
                Vector::new(10.0, 400.0),
            )?;

            font.draw( 
                &mut gfx,
                &format!("Cards left: {} (hand: {} deck: {})", self.hand.len() + self.deck.len(),
                         self.hand.len(), self.deck.len()),
                Color::RED,
                Vector::new(10.0, 450.0),
            )?;

            font.draw( 
                &mut gfx,
                &format!("Total:      {}", self.payments * 3 
                         + self.trophies * 2 
                         + self.hand.len() as u32 + self.deck.len() as u32),
                Color::RED,
                Vector::new(10.0, 500.0),
            )?;

            return gfx.present(&window);
        }

        // Start row 1 from `PADDING` from the top
        let mut curr_y = PADDING;

        // Calculate the regions that are clickable from the drawing
        self.clickables.clear();

        /* Row 1 */
        // Get the card type for the current player
        let image = match self.player_kind {
            PlayerKind::Regular => &self.images[&AssetType::RegPlayer],
            PlayerKind::Monstrous =>     &self.images[&AssetType::MonstrousPlayer],
        };

        // Calculate the X coord based on the player index
        let image_width = image.size().x;
        let curr_x = PADDING + (image_width + PADDING) * self.player_index as f32;

        // Draw the player image in Row 1
        let region = Rectangle::new(Vector::new(curr_x, curr_y), image.size());
        gfx.draw_image(&image, region);

        // Draw the action buttons on the left/right side of the player
        let range_target_image = &self.images[&AssetType::RangeTarget];
        let range_target_size = range_target_image.size();
        let melee_target_image = &self.images[&AssetType::MeleeTarget];
        let melee_target_size = melee_target_image.size();
        let move_target_image = &self.images[&AssetType::MoveTarget];
        let move_target_size = move_target_image.size();
        let swap_target_image = &self.images[&AssetType::SwapTarget];
        let swap_target_size = swap_target_image.size();
        let reign_target_image = &self.images[&AssetType::ReignTarget];
        let reign_target_size = reign_target_image.size() * 0.2;

        if self.player_index > 0 {
            // Draw action clickables on the right side of the player
            let region = Rectangle::new(
                Vector::new(curr_x - range_target_size.x / 2.0, 
                            curr_y + image.size().y * 0.20), 
                range_target_size);
            gfx.stroke_rect(&region, Color::WHITE);
            gfx.draw_image(&range_target_image, region);

            // Add this action to available clickables
            self.clickables.push((region, 
                ClickableType::Action(Action::Range(Entity::Character, Direction::Left))));

            // Draw action clickables on the right side of the player
            let region = Rectangle::new(
                Vector::new(curr_x - move_target_size.x / 2.0, 
                            curr_y + image.size().y * 0.50), 
                range_target_size);
            gfx.stroke_rect(&region, Color::WHITE);
            gfx.draw_image(&move_target_image, region);

            // Add this action to available clickables
            self.clickables.push((region, 
                ClickableType::Action(Action::Move(Entity::Character, Direction::Left))));
        }

        if self.player_index < MONSTER_DECK_SIZE {
            // Draw action clickables on the right side of the player
            let region = Rectangle::new(
                Vector::new(curr_x - range_target_size.x / 2.0 + image_width, 
                            curr_y + image.size().y * 0.20), 
                range_target_size);
            gfx.stroke_rect(&region, Color::WHITE);
            gfx.draw_image(&range_target_image, region);

            // Add this action to available clickables
            self.clickables.push((region, 
                ClickableType::Action(Action::Range(Entity::Character, Direction::Right))));

            // Draw action clickables on the right side of the player
            let region = Rectangle::new(
                Vector::new(curr_x - move_target_size.x / 2.0 + image_width, 
                            curr_y + image.size().y * 0.50), 
                move_target_size);
            gfx.stroke_rect(&region, Color::WHITE);
            gfx.draw_image(&move_target_image, region);

            // Add this action to available clickables
            self.clickables.push((region, 
                ClickableType::Action(Action::Move(Entity::Character, Direction::Right))));
        }

        // Draw action clickables on the right side of the player
        let region = Rectangle::new(
            Vector::new(curr_x - melee_target_size.x / 2.0 + image_width / 2.0, 
                        curr_y + image.size().y - melee_target_size.y), 
            move_target_size);
        gfx.stroke_rect(&region, Color::WHITE);
        gfx.draw_image(&melee_target_image, region);

        // Add this action to available clickables
        self.clickables.push((region, 
            ClickableType::Action(Action::Melee(Entity::Character))));

        /* End Row 1 */

        // Adjust the row to the second row
        curr_y += image.size().y + PADDING;

        // Get the current font
        let mut font = self.font.to_renderer(&gfx, 24.0)?;

        /* Row 2 */
        let mut curr_x = PADDING;
        let mut monster_image_width = None;
        for monster_index in 0..MONSTER_DECK_SIZE {
            // Draw quality of life indexes above monsters on character side to allow for easier 
            // count
            let player_offset = (self.player_index as isize - monster_index as isize).abs();
            if player_offset > 0 && player_offset <= 5 {
                font.draw( 
                    &mut gfx,
                    &format!("{}", player_offset),
                    Color::WHITE,
                    Vector::new(curr_x, curr_y),
                )?;
            }

            // Get the image of the monster based if it is alive or dead
            let image = match self.monsters.alive[monster_index] {
                true  => &self.monsters.images[monster_index],
                false => &self.images[&AssetType::CardBack]
            };

            let image_size = image.size();

            // Draw quality of life indexes above monsters on character side to allow for easier 
            // count
            let companion_offset = (self.companion_index as isize - monster_index as isize).abs();
            if companion_offset > 0 && companion_offset <= 5 {
                font.draw( 
                    &mut gfx,
                    &format!("{}", companion_offset),
                    Color::WHITE,
                    Vector::new(curr_x, curr_y + image.size().y + PADDING * 1.5),
                )?;
            }

            if monster_image_width.is_none() {
                monster_image_width = Some(image_size);
            }

            // Draw each image in Row 2
            let region = Rectangle::new(Vector::new(curr_x, curr_y), image_size);
            gfx.draw_image(&image, region);

            // Draw each of the current hits on each monster
            for (i, to_slay) in self.monsters.current_hits[monster_index].iter().enumerate() {
                // Get the token image
                let target_image = match to_slay {
                    ToSlay::Melee => melee_target_image,
                    ToSlay::Range => range_target_image,
                    ToSlay::Move  => move_target_image,
                };

                // Calculate the location of the middle of the monster card
                let region = Rectangle::new(
                    Vector::new(curr_x + image.size().x * 0.5 - target_image.size().x * 0.5, 
                                curr_y + image.size().y * 0.2 + 
                                    i as f32 * (PADDING + target_image.size().y)), 
                                target_image.size());

                // Draw the ToSlay image in the middle of the Monster
                gfx.draw_image(&target_image, region);
                gfx.stroke_rect(&region, Color::BLUE);
            }

            // Draw the strength adjustment if it is there for each monster
            if self.monsters.alive[monster_index] {
                let adjustment = self.monsters.strength_adjustments[monster_index];
                if adjustment > 0 {
                    font.draw( 
                        &mut gfx,
                        &format!("+{}", adjustment),
                        Color::RED,
                        Vector::new(curr_x + 20.0, curr_y + 20.0),
                    )?;
                }
            }

            if matches!(self.monsters.abilities[monster_index], Some(Ability::Reign)) && 
                    self.monsters.alive[monster_index] {
                // Display Reign tooltip next to a monster that needs to be killed before the 
                // current monster can be killed
                if monster_index > 0 && self.monsters.alive[monster_index - 1] {
                    let left_strength = self.monsters.strengths[monster_index - 1]
                        + self.monsters.strength_adjustments[monster_index - 1];
                    let curr_strength = self.monsters.strengths[monster_index]
                        + self.monsters.strength_adjustments[monster_index];
                    if left_strength < curr_strength {
                        let region = Rectangle::new(
                            Vector::new(curr_x, 
                                        curr_y + image.size().y * 0.5 - reign_target_size.y * 0.5), 
                            reign_target_size);
                        gfx.draw_image(&reign_target_image, region);
                        gfx.stroke_rect(&region, Color::BLACK);
                    }
                }

                // Display Reign tooltip next to a monster that needs to be killed before the 
                // current monster can be killed
                if monster_index < (MONSTER_DECK_SIZE - 1) 
                    && self.monsters.alive[monster_index + 1] {
                    let right_strength = self.monsters.strengths[monster_index + 1]
                        + self.monsters.strength_adjustments[monster_index + 1];
                    let curr_strength = self.monsters.strengths[monster_index]
                        + self.monsters.strength_adjustments[monster_index];
                    if right_strength < curr_strength {
                        let region = Rectangle::new(
                            Vector::new(curr_x + image.size().x - reign_target_size.x, 
                                        curr_y + image.size().y * 0.5 - reign_target_size.y * 0.5), 
                            reign_target_size);
                        gfx.draw_image(&reign_target_image, region);
                        gfx.stroke_rect(&region, Color::BLACK);
                    }
                }
            }

            // Update curr_x for the next monster
            let width = image_size.x;
            curr_x += PADDING + width;
        }
        /* End Row 2 */

        assert!(monster_image_width.is_some());

        // Adjust the row to the third row
        curr_y += monster_image_width.unwrap().y + PADDING;

        /* Row 3 */
        let image = match self.companion_kind {
            CompanionKind::Melee => &self.images[&AssetType::MeleeCompanion],
            CompanionKind::Range => &self.images[&AssetType::RangeCompanion],
        };

        // Calculate the X coord based on the player index
        let image_width = image.size().x;
        let curr_x = PADDING + (image_width + PADDING) * self.companion_index as f32;

        // Draw the player image in Row 1
        let region = Rectangle::new(Vector::new(curr_x, curr_y), image.size());
        gfx.draw_image(&image, region);

        if self.companion_index > 0 {
            if matches!(self.companion_kind, CompanionKind::Range) {
                // If the companion is range, draw the range action button on the left side
                let region = Rectangle::new(
                    Vector::new(curr_x - range_target_size.x / 2.0, 
                                curr_y + image.size().y * 0.20), 
                    range_target_size);
                gfx.stroke_rect(&region, Color::WHITE);
                gfx.draw_image(&range_target_image, region);

                // Add this action to available clickables
                self.clickables.push((region, 
                    ClickableType::Action(Action::Range(Entity::Companion, Direction::Left))));
            }

            // Draw the move action on the left of the companion
            let region = Rectangle::new(
                Vector::new(curr_x - move_target_size.x / 2.0, 
                            curr_y + image.size().y * 0.50), 
                range_target_size);
            gfx.stroke_rect(&region, Color::WHITE);
            gfx.draw_image(&move_target_image, region);

            // Add this action to available clickables
            self.clickables.push((region, 
                ClickableType::Action(Action::Move(Entity::Companion, Direction::Left))));

        }

        if self.companion_index < MONSTER_DECK_SIZE {
            // If the companion is range, draw the range action button on the right side
            if matches!(self.companion_kind, CompanionKind::Range) {
                let region = Rectangle::new(
                    Vector::new(curr_x - range_target_size.x / 2.0 + image_width, 
                                curr_y + image.size().y * 0.20), 
                    range_target_size);
                gfx.stroke_rect(&region, Color::WHITE);
                gfx.draw_image(&range_target_image, region);

                // Add this action to available clickables
                self.clickables.push((region, 
                    ClickableType::Action(Action::Range(Entity::Companion, Direction::Right))));
            }

            // Draw the move action on the right of the companion
            let region = Rectangle::new(
                Vector::new(curr_x - move_target_size.x / 2.0 + image_width, 
                            curr_y + image.size().y * 0.50), 
                move_target_size);
            gfx.stroke_rect(&region, Color::WHITE);
            gfx.draw_image(&move_target_image, region);

            // Add this action to available clickables
            self.clickables.push((region, 
                ClickableType::Action(Action::Move(Entity::Companion, Direction::Right))));
        }


        // If companion is melee, draw the melee action button
        if matches!(self.companion_kind, CompanionKind::Melee) {
            let region = Rectangle::new(
                Vector::new(curr_x - melee_target_size.x / 2.0 + image_width / 2.0, 
                            curr_y), 
                move_target_size);
            gfx.stroke_rect(&region, Color::WHITE);
            gfx.draw_image(&melee_target_image, region);

            // Add this action to available clickables
            self.clickables.push((region, 
                ClickableType::Action(Action::Melee(Entity::Companion))));
        }

        // Draw the swap action button
        let region = Rectangle::new(
            Vector::new(curr_x - swap_target_size.x / 2.0 + image_width / 2.0, 
                        curr_y + image.size().y - swap_target_size.y), 
            swap_target_size);
        gfx.stroke_rect(&region, Color::WHITE);
        gfx.draw_image(&swap_target_image, region);

        // Add this action to available clickables
        self.clickables.push((region, ClickableType::Action(Action::Swap)));

        /* End Row 3 */

        // Adjust the row to the fourth row
        curr_y += monster_image_width.unwrap().y + PADDING;

        /* Row 4 */

        let mut curr_x = PADDING;

        // Alaways display the hand of cards in sorted order
        self.hand.sort();

        let mut row_4_image_width = 0.0;
        // Draw the hand of cards
        for (i, card) in self.hand.iter().enumerate() {
            let image = &self.images[&AssetType::Action(*card)];
            let image_size = image.size();
            if row_4_image_width == 0.0 {
                row_4_image_width = image_size.x;
            }

            // Draw each action card
            let region = Rectangle::new(Vector::new(curr_x, curr_y), image_size);
            gfx.draw_image(&image, region);

            // Add this card to available clickables
            self.clickables.push((region, ClickableType::Card(i)));

            // Update the column to the next column
            curr_x += image.size().x + PADDING;
        }

        let curr_x = PADDING  + (row_4_image_width + PADDING) * 6.0;
        let mut font = self.font.to_renderer(&gfx, 34.0)?;
        let region = Rectangle::new(Vector::new(curr_x, curr_y), 
                                    Vector::new(image.size().x, image.size().y / 4.0));

        gfx.fill_rect(&region, Color::WHITE);
        gfx.stroke_rect(&region, Color::GREEN);

        // Add this card to available clickables
        self.clickables.push((region, ClickableType::Action(Action::EndTurn)));

        // Draw the font
        font.draw( 
            &mut gfx,
            &format!("End turn"),
            Color::BLACK,
            Vector::new(curr_x + 3.0, curr_y + image.size().y / 4.0 - PADDING),
        )?;

        let mut font = self.font.to_renderer(&gfx, 48.0)?;
        font.draw( 
            &mut gfx,
            &format!("Deck left: {}", self.deck.len()),
            Color::WHITE,
            Vector::new(curr_x + 3.0, curr_y + image.size().y * 0.75),
        )?;

        font.draw( 
            &mut gfx,
            &format!("Trophies: {}", self.trophies),
            Color::WHITE,
            Vector::new(curr_x + 3.0, curr_y + image.size().y * 1.0),
        )?;


        gfx.present(&window)
    }

    pub fn update(&mut self, location: Vector) {
        for (region, new_action) in self.clickables.iter() {
            if region.contains(location) {
                match new_action {
                    ClickableType::Action(action) => self.current_action = Some(*action),
                    ClickableType::Card(card) => self.current_card = Some(*card),
                    ClickableType::State(State::Reset) => {
                        self.state = State::Reset;
                        return;
                    }
                    ClickableType::State(_) => {}
                }
            }
        }

        // Variables set if an action is valid
        let mut current_monster = None;
        let mut reset = false;
        let mut add_trophy = false;

        // If we have selected a card and an action, perform the logic for that request
        match (self.current_action, self.current_card) {
            // Movement action
            (Some(Action::Move(entity, direction)), Some(hand_index)) => {
                assert!(hand_index < self.hand.len(), 
                    "Move: Given hand_size {} larger than hand.len() {}", hand_index, 
                    self.hand.len());
                let num = self.hand.remove(hand_index) as usize;

                let index = match (entity, direction) {
                    (Entity::Character, Direction::Left) => {
                        self.player_index = self.player_index.saturating_sub(num);
                        info!("New player index left: {}", self.player_index);
                        self.player_index
                    }
                    (Entity::Character, Direction::Right) => {
                        let mut new_index = self.player_index + num;
                        if new_index >= MONSTER_DECK_SIZE {
                            new_index = MONSTER_DECK_SIZE - 1;
                        }
                        self.player_index = new_index;
                        info!("New player index right: {}", self.player_index);
                        self.player_index
                    }
                    (Entity::Companion, Direction::Left) => {
                        self.companion_index = self.companion_index.saturating_sub(num);
                        info!("New companion index: {}", self.companion_index);
                        self.companion_index
                    }
                    (Entity::Companion, Direction::Right) => {
                        let mut new_index = self.companion_index + num;
                        if new_index >= MONSTER_DECK_SIZE {
                            new_index = MONSTER_DECK_SIZE - 1;
                        }

                        self.companion_index = new_index;
                        info!("New companion index: {}", self.companion_index);
                        self.companion_index
                    }
                };

                // Add the ToSlay marker to the moved to monster
                self.monsters.current_hits[index].push(ToSlay::Move);

                if num as u8 == (self.monsters.strengths[index] 
                                 + self.monsters.strength_adjustments[index]) {
                    add_trophy = true;
                }

                // Moving onto a Noxious monster results in randomly losing a card
                if matches!(self.monsters.abilities[index], Some(Ability::Noxious)) 
                        && self.monsters.alive[index] {
                    self.hand.remove(rand::random::<usize>() % self.hand.len());
                    self.discarded = true;
                }

                current_monster = Some(index);

                // Reset the chosen card and action
                self.current_card   = None;
                self.current_action = None;
            }
            // Swap action
            (Some(Action::Swap), Some(hand_index)) => {
                // Ensure our hand_index is in bounds
                assert!(hand_index < self.hand.len(), 
                    "Swap: Given hand_size {} larger than hand.len() {}", 
                    hand_index, self.hand.len());

                // Remove the card from the hand
                let _ = self.hand.remove(hand_index) as usize;

                // Change the companion to the other kind
                self.companion_kind = match self.companion_kind {
                    CompanionKind::Melee => CompanionKind::Range,
                    CompanionKind::Range => CompanionKind::Melee,
                };

                // Reset the chosen card and action
                self.current_card   = None;
                self.current_action = None;
            }
            // Range action with Companion to the left
            (Some(Action::Range(entity, Direction::Left)), Some(hand_index)) => {
                // Ensure our hand_index is in bounds
                assert!(hand_index < self.hand.len(), 
                    "RangeLeft: Given hand_size {} larger than hand.len() {}", 
                    hand_index, self.hand.len());

                // Remove the card from the hand
                let num = self.hand.remove(hand_index) as usize;

                match entity {
                    Entity::Companion => {
                        // Ensure we are in bounds for the range attack
                        if matches!(self.companion_index.checked_sub(num), Some(0..13)) {
                            info!("Range LEFT Companion {} hitting {}", num, self.monsters.names[num]);
                            self.monsters.current_hits[self.companion_index - num]
                                .push(ToSlay::Range);

                            current_monster = Some(self.companion_index - num);

                            let monster_str = self.monsters.strengths[self.companion_index - num]
                                + self.monsters.strength_adjustments[self.companion_index - num];
                            if num as u8 == monster_str {
                                add_trophy = true;
                            }
                        }
                    }
                    Entity::Character => {
                        // Ensure we are in bounds for the range attack
                        if matches!(self.player_index.checked_sub(num), Some(0..13)) {
                            info!("Range LEFT Character {} hitting {}", num, self.monsters.names[num]);
                            self.monsters.current_hits[self.player_index - num]
                                .push(ToSlay::Range);

                            current_monster = Some(self.player_index - num);

                            let monster_str = self.monsters.strengths[self.player_index - num]
                                + self.monsters.strength_adjustments[self.player_index - num];
                            if num as u8 == monster_str {
                                add_trophy = true;
                            }
                        }
                    }
                }
                // Reset the chosen card and action
                self.current_card   = None;
                self.current_action = None;
            }
            // Range action with Companion to the Right
            (Some(Action::Range(entity, Direction::Right)), Some(hand_index)) => {
                // Ensure our hand_index is in bounds
                assert!(hand_index < self.hand.len(), 
                    "RangeRight: Given hand_size {} larger than hand.len() {}", 
                    hand_index, self.hand.len());

                // Remove the card from the hand
                let num = self.hand.remove(hand_index) as usize;

                match entity {
                    Entity::Companion => {
                        // Ensure companion are in bounds for the range attack
                        if self.companion_index + num < MONSTER_DECK_SIZE {
                            info!("Range RIGHT {} hitting {}", num, self.monsters.names[num]);
                            self.monsters.current_hits[self.companion_index + num]
                                .push(ToSlay::Range);
                            current_monster = Some(self.companion_index + num);

                            let monster_str = self.monsters.strengths[self.companion_index + num]
                                + self.monsters.strength_adjustments[self.companion_index + num];
                            if num as u8 == monster_str {
                                add_trophy = true;
                            }
                        }
                    }
                    Entity::Character => {
                        // Ensure character are in bounds for the range attack
                        if self.player_index + num < MONSTER_DECK_SIZE {
                            info!("Range RIGHT {} hitting {}", num, self.monsters.names[num]);
                            self.monsters.current_hits[self.player_index + num]
                                .push(ToSlay::Range);
                            current_monster = Some(self.player_index + num);

                            let monster_str = self.monsters.strengths[self.player_index + num]
                                + self.monsters.strength_adjustments[self.player_index + num];
                            if num as u8 == monster_str {
                                add_trophy = true;
                            }
                        }
                    }
                }

                // Reset the chosen card and action
                self.current_card   = None;
                self.current_action = None;
            }
            (Some(Action::Melee(entity)), Some(hand_index)) => {
                // Ensure our hand_index is in bounds
                assert!(hand_index < self.hand.len(), 
                    "Melee: Given hand_size {} larger than hand.len() {}", 
                    hand_index, self.hand.len());

                // Remove the card from the hand
                let num = self.hand.remove(hand_index);

                // Get the monster index based on the entity using Melee
                let monster_index = match entity {
                    Entity::Character => self.player_index,
                    Entity::Companion => self.companion_index,
                };

                // Get the current monster strength
                let monster_strength = self.monsters.strengths[monster_index] 
                    + self.monsters.strength_adjustments[monster_index];

                // If the action card number is greater than or equal to the monster strength,
                // it is a successful melee attack
                if num >= monster_strength {
                    self.monsters.current_hits[monster_index].push(ToSlay::Melee);
                    current_monster = Some(monster_index);

                    if num == monster_strength {
                        add_trophy = true;
                    }
                }

                // Reset the chosen card and action
                self.current_card   = None;
                self.current_action = None;
            }
            (Some(Action::EndTurn), _) => {
                reset = true;

                // Reset the chosen card and action
                self.current_card   = None;
                self.current_action = None;
            }
            _ => { }
        }

        info!("Current actions: {:?} {:?}", self.current_action, self.current_card);

        // Check if the current monster is dead by removing all elements from the 
        // current hit Vector from the to_slay Vector. If at the end of that, the 
        // to_slays vector is empty, then that monster is dead.
        if let Some(index) = current_monster {
            if self.monsters.alive[index] {
                let curr_hits = &self.monsters.current_hits[index];
                let mut to_slays  = self.monsters.to_slays[index].clone();

                // Remove all current hits from the to_slays vec
                for curr_hit in curr_hits {
                    to_slays.remove_item(&curr_hit);
                }

                // If to_slays is empty, we have enough hits for the monster to be dead
                if to_slays.len() == 0 {
                    self.monsters.alive[index] = false;
                    self.monsters.current_hits[index].clear();

                    // If the last action resulted in a trophy, add it
                    if add_trophy {
                        self.trophies += 1;
                    }
                }
            }
        }

        // Reset all strength adjustments
        for index in 0..MONSTER_DECK_SIZE {
            self.monsters.strength_adjustments[index] = 0;
        }

        // Adjust the strength_adjustments for Rally monsters if that monster is alive
        for index in 0..MONSTER_DECK_SIZE {
            if matches!(self.monsters.abilities[index], Some(Ability::Rally)) 
                    && self.monsters.alive[index] {
                info!("Rally {}", index);
                if index > 0 {
                    self.monsters.strength_adjustments[index - 1] += 1;
                }

                if index < (MONSTER_DECK_SIZE - 1) {
                    self.monsters.strength_adjustments[index + 1] += 1;
                }
            }
        }

        // We are out of cards in hand and should reset
        if self.hand.len() == 0 && !self.discarded {
            reset = true;

            // If we ran out of cards then we can always say the player is Monstrous
            self.hand_limit = 6;
            self.player_kind = PlayerKind::Monstrous;

        }

        if reset {
            // Clear the hand to be replinished
            self.hand.clear();

            // Replinish hand of cards
            for _ in 0..self.hand_limit {
                if let Some(new_card) = self.deck.pop() {
                    self.hand.push(new_card);
                }
            }

            // Reset the current hits on all monsters
            for curr_hit in self.monsters.current_hits.iter_mut() {
                curr_hit.clear();
            }

        }

        // End game is triggered when no cards in hand and no cards left in the deck
        if self.hand.len() == 0 && self.deck.len() == 0 {
            self.state = State::EndGame;
        }

        self.discarded = false;
    }
}

// This time we might return an error, so we use a Result
async fn app(window: Window, mut gfx: Graphics, mut input: Input) -> Result<()> {
    // Top of the reset loop. We will continue from 'reset_game when we get a reset game state
    'reset_game: loop {
        // Display the loading screen
        gfx.clear(Color::BLACK);
        let mut font = VectorFont::load("iosevka-regular.ttf").await?.to_renderer(&gfx, 72.0)?;
        font.draw(&mut gfx, "Loading Maverick...", Color::RED, Vector::new(10.0, 150.0))?;
        gfx.present(&window)?;

        // Initialize this game
        let mut game = Game::init(&gfx).await?;

        // Initial update
        game.update(Vector::new(0.0, 0.0));

        loop {
            // let mut location = None;
            while let Some(event) = input.next_event().await {
                match event {
                    Event::PointerMoved(_e) => {
                        // location = Some(e.location());
                    }
                    Event::PointerInput(e) => {
                        if !e.is_down() {
                            continue;
                        }

                        game.update(input.mouse().location());
                    }
                    _ => {
                        info!("Skipping.. {:?}", event);
                        continue;
                    }
                }
            }

            if matches!(game.state, State::Reset) {
                continue 'reset_game;
            }

            gfx.clear(Color::BLACK);

            // Draw the current game state and populate the clickables to highlight in the UI
            game.draw(&window, &mut gfx).await?;

            // Highlight each clickable found in `draw()`
            for (region, action) in &game.clickables {
                match action {
                    ClickableType::Action(curr_action) => {
                        if Some(curr_action) == game.current_action.as_ref() {
                            gfx.stroke_rect(&region, Color::RED);
                        } else {
                            gfx.stroke_rect(&region, Color::GREEN);
                        }
                    }
                    ClickableType::Card(index) => {
                        if Some(index) == game.current_card.as_ref() {
                            gfx.stroke_rect(&region, Color::RED);
                        } else {
                            gfx.stroke_rect(&region, Color::GREEN);
                        }
                    }
                    _ => gfx.stroke_rect(&region, Color::GREEN)
                }

                gfx.present(&window)?;
            }
        }
    }
}

