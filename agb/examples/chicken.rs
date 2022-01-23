#![no_std]
#![no_main]

use agb::{
    display::{
        background::{RegularMap, TileFormat, TileSet, TileSetting},
        object::ObjectStandard,
        HEIGHT, WIDTH,
    },
    input::Button,
};
use core::convert::TryInto;

#[derive(PartialEq, Eq)]
enum State {
    Ground,
    Upwards,
    Flapping,
}

struct Character<'a> {
    object: ObjectStandard<'a>,
    position: Vector2D,
    velocity: Vector2D,
}

struct Vector2D {
    x: i32,
    y: i32,
}

fn tile_is_collidable(tile: u16) -> bool {
    let masked = tile & 0b0000001111111111;
    masked == 0 || masked == 4
}

fn frame_ranger(count: u32, start: u32, end: u32, delay: u32) -> u16 {
    (((count / delay) % (end + 1 - start)) + start) as u16
}

#[agb::entry]
fn main(mut gba: agb::Gba) -> ! {
    let map_as_grid: &[[u16; 32]; 32] = unsafe {
        (&MAP_MAP as *const [u16; 1024] as *const [[u16; 32]; 32])
            .as_ref()
            .unwrap()
    };

    let mut gfx = gba.display.video.tiled0();
    let vblank = agb::interrupt::VBlank::get();
    let mut input = agb::input::ButtonController::new();

    gfx.vram.set_background_palette_raw(&MAP_PALETTE);
    let tileset = TileSet::new(&MAP_TILES, TileFormat::FourBpp);
    let tileset_ref = gfx.vram.add_tileset(tileset);

    let mut background = gfx.background();

    for (i, &tile) in MAP_MAP.iter().enumerate() {
        let i = i as u16;
        background.set_tile(
            &mut gfx.vram,
            (i % 32, i / 32).into(),
            tileset_ref,
            tile & ((1 << 10) - 1),
            TileSetting::from_raw(tile),
        );
    }

    background.show();
    background.commit();

    let mut object = gba.display.object.get();

    object.set_sprite_palette_raw(&CHICKEN_PALETTE);
    object.set_sprite_tilemap(&CHICKEN_TILES);

    object.enable();
    let mut chicken = Character {
        object: object.get_object_standard(),
        position: Vector2D {
            x: (6 * 8) << 8,
            y: ((7 * 8) - 4) << 8,
        },
        velocity: Vector2D { x: 0, y: 0 },
    };

    chicken.object.set_tile_id(0);
    chicken
        .object
        .set_x((chicken.position.x >> 8).try_into().unwrap());
    chicken
        .object
        .set_y((chicken.position.y >> 8).try_into().unwrap());
    chicken.object.show();
    chicken.object.commit();

    let acceleration = 1 << 4;
    let gravity = 1 << 4;
    let flapping_gravity = gravity / 3;
    let jump_velocity = 1 << 9;
    let mut frame_count = 0;
    let mut frames_off_ground = 0;

    let terminal_velocity = (1 << 8) / 2;

    loop {
        vblank.wait_for_vblank();
        frame_count += 1;

        input.update();

        // Horizontal movement
        chicken.velocity.x += (input.x_tri() as i32) * acceleration;
        chicken.velocity.x = 61 * chicken.velocity.x / 64;

        // Update position based on collision detection
        let state = handle_collision(
            &mut chicken,
            map_as_grid,
            gravity,
            flapping_gravity,
            terminal_velocity,
        );

        if state != State::Ground {
            frames_off_ground += 1;
        } else {
            frames_off_ground = 0;
        }

        // Jumping code
        if frames_off_ground < 10 && input.is_just_pressed(Button::A) {
            frames_off_ground = 200;
            chicken.velocity.y = -jump_velocity;
        }

        restrict_to_screen(&mut chicken);
        update_chicken_object(&mut chicken, state, frame_count);

        // Commit the chicken to vram
        chicken.object.commit();
    }
}

fn update_chicken_object(chicken: &mut Character, state: State, frame_count: u32) {
    if chicken.velocity.x > 1 {
        chicken.object.set_hflip(false);
    } else if chicken.velocity.x < -1 {
        chicken.object.set_hflip(true);
    }
    match state {
        State::Ground => {
            if chicken.velocity.x.abs() > 1 << 4 {
                chicken
                    .object
                    .set_tile_id(frame_ranger(frame_count, 1, 3, 10));
            } else {
                chicken.object.set_tile_id(0);
            }
        }
        State::Upwards => {}
        State::Flapping => {
            chicken
                .object
                .set_tile_id(frame_ranger(frame_count, 4, 5, 5));
        }
    }

    let x: u16 = (chicken.position.x >> 8).try_into().unwrap();
    let y: u16 = (chicken.position.y >> 8).try_into().unwrap();

    chicken.object.set_x(x - 4);
    chicken.object.set_y(y - 4);
}

fn restrict_to_screen(chicken: &mut Character) {
    if chicken.position.x > (WIDTH - 8 + 4) << 8 {
        chicken.velocity.x = 0;
        chicken.position.x = (WIDTH - 8 + 4) << 8;
    } else if chicken.position.x < 4 << 8 {
        chicken.velocity.x = 0;
        chicken.position.x = 4 << 8;
    }
    if chicken.position.y > (HEIGHT - 8 + 4) << 8 {
        chicken.velocity.y = 0;
        chicken.position.y = (HEIGHT - 8 + 4) << 8;
    } else if chicken.position.y < 4 << 8 {
        chicken.velocity.y = 0;
        chicken.position.y = 4 << 8;
    }
}

fn handle_collision(
    chicken: &mut Character,
    map_as_grid: &[[u16; 32]; 32],
    gravity: i32,
    flapping_gravity: i32,
    terminal_velocity: i32,
) -> State {
    let mut new_chicken_x = chicken.position.x + chicken.velocity.x;
    let mut new_chicken_y = chicken.position.y + chicken.velocity.y;

    let tile_x = ((new_chicken_x >> 8) / 8) as usize;
    let tile_y = ((new_chicken_y >> 8) / 8) as usize;

    let left = (((new_chicken_x >> 8) - 4) / 8) as usize;
    let right = (((new_chicken_x >> 8) + 4) / 8) as usize;
    let top = (((new_chicken_y >> 8) - 4) / 8) as usize;
    let bottom = (((new_chicken_y >> 8) + 4) / 8) as usize;

    if chicken.velocity.x < 0 && tile_is_collidable(map_as_grid[tile_y][left]) {
        new_chicken_x = (((left + 1) * 8 + 4) << 8) as i32;
        chicken.velocity.x = 0;
    } else if chicken.velocity.x > 0 && tile_is_collidable(map_as_grid[tile_y][right]) {
        new_chicken_x = ((right * 8 - 4) << 8) as i32;
        chicken.velocity.x = 0;
    }

    if chicken.velocity.y < 0 && tile_is_collidable(map_as_grid[top][tile_x]) {
        new_chicken_y = ((((top + 1) * 8 + 4) << 8) + 4) as i32;
        chicken.velocity.y = 0;
    } else if chicken.velocity.y > 0 && tile_is_collidable(map_as_grid[bottom][tile_x]) {
        new_chicken_y = ((bottom * 8 - 4) << 8) as i32;
        chicken.velocity.y = 0;
    }

    let mut air_animation = State::Ground;

    if !tile_is_collidable(map_as_grid[bottom][tile_x]) {
        if chicken.velocity.y < 0 {
            air_animation = State::Upwards;
            chicken.velocity.y += gravity;
        } else {
            air_animation = State::Flapping;
            chicken.velocity.y += flapping_gravity;
            if chicken.velocity.y > terminal_velocity {
                chicken.velocity.y = terminal_velocity;
            }
        }
    }

    chicken.position.x = new_chicken_x;
    chicken.position.y = new_chicken_y;

    air_animation
}

// Below is the data for the sprites

static CHICKEN_TILES: [u32; 8 * 6] = [
    0x01100000, 0x11100000, 0x01100010, 0x01111110, 0x01111110, 0x00001000, 0x00001000, 0x00011000,
    0x01100000, 0x11100000, 0x01100010, 0x01111110, 0x01111110, 0x00010100, 0x00100100, 0x00000010,
    0x01100000, 0x11100000, 0x01100010, 0x01111110, 0x01111110, 0x00011000, 0x00100110, 0x00100000,
    0x01100000, 0x11100000, 0x01100010, 0x01111110, 0x01111110, 0x00011000, 0x00011100, 0x00001000,
    0x01100000, 0x11111100, 0x01111010, 0x01111110, 0x01111110, 0x00011000, 0x00010000, 0x00000000,
    0x01100000, 0x11100000, 0x01111110, 0x01111110, 0x01111110, 0x00011000, 0x00010000, 0x00000000,
];

static CHICKEN_PALETTE: [u16; 1] = [0x7C1E];

static MAP_TILES: [u32; 8 * 17] = [
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111,
    0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x01111111, 0x01111111, 0x01111111,
    0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x00000000, 0x01010101, 0x01010101,
    0x00000000, 0x00000000, 0x11110000, 0x11100000, 0x11000100, 0x10001100, 0x00011100, 0x00111100,
    0x00000000, 0x00000000, 0x01110001, 0x01100011, 0x01000111, 0x00001111, 0x00011111, 0x00111111,
    0x00111111, 0x00111111, 0x00111111, 0x00111111, 0x00111111, 0x00111111, 0x00001111, 0x00000111,
    0x00000000, 0x00000000, 0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111,
    0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x10111111, 0x01101011,
    0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x01101011,
    0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11101111, 0x10110111,
    0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11101111,
    0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111101, 0x11111011, 0x10111011,
    0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x10111011,
    0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11101111, 0x11101011,
    0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11011111, 0x11010111,
    0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x11111111, 0x01111111, 0x11010111,
];

static MAP_MAP: [u16; 1024] = [
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0002, 0x0003, 0x0003, 0x0003, 0x0003,
    0x0003, 0x0003, 0x0003, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0004,
    0x0404, 0x0004, 0x0404, 0x0004, 0x0404, 0x0004, 0x0404, 0x0004, 0x0404, 0x0004, 0x0404, 0x0001,
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0804, 0x0C04, 0x0804, 0x0C04, 0x0804,
    0x0C04, 0x0804, 0x0C04, 0x0804, 0x0C04, 0x0804, 0x0C04, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    0x0001, 0x0001, 0x0001, 0x0005, 0x0405, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    0x0001, 0x0005, 0x0405, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0006,
    0x0406, 0x0002, 0x0003, 0x0003, 0x0003, 0x0003, 0x0003, 0x0003, 0x0001, 0x0006, 0x0406, 0x0001,
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0000, 0x0000, 0x0004, 0x0404, 0x0004,
    0x0404, 0x0004, 0x0404, 0x0004, 0x0404, 0x0000, 0x0000, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    0x0001, 0x0001, 0x0001, 0x0000, 0x0000, 0x0007, 0x0007, 0x0007, 0x0007, 0x0007, 0x0007, 0x0007,
    0x0007, 0x0000, 0x0000, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0008, 0x0009, 0x000A, 0x0000,
    0x0000, 0x000B, 0x000C, 0x000D, 0x000B, 0x000E, 0x0008, 0x0009, 0x000A, 0x0000, 0x0000, 0x000B,
    0x000B, 0x000C, 0x000D, 0x000B, 0x000E, 0x0008, 0x0009, 0x000A, 0x000F, 0x0010, 0x000B, 0x000C,
    0x000D, 0x000B, 0x000E, 0x0008, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000,
];

static MAP_PALETTE: [u16; 2] = [0x0000, 0x6A2F];
