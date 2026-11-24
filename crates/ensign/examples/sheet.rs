//! Paint a contact sheet of generated flags.
//!
//! `cargo run -p shipvivor-ensign --example sheet -- [first seed] [rows]`
//!
//! Each row is one seed: the flag drawn from it on the left, then four rivals drawn
//! against it, as one run's enemy would hoist on successive legs. The sheet goes to
//! `work/flags/sheet.png` and each flag's description is printed.
use shipvivor_ensign::Flag;

const WIDTH: u32 = 140;
const HEIGHT: u32 = 50;
const GUTTER: u32 = 14;
const RIVALS: u32 = 4;
const BACKGROUND: [u8; 3] = [43, 58, 66];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let first: u64 = args.first().and_then(|s| s.parse().ok()).unwrap_or(1);
    let rows: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(12);
    let columns = 1 + RIVALS;
    // A wider gutter separates each seed's own flag from its rivals.
    let sheet_width = GUTTER * 3 + columns * (WIDTH + GUTTER);
    let sheet_height = GUTTER + rows * (HEIGHT + GUTTER);
    let mut rgb: Vec<u8> = BACKGROUND
        .iter()
        .copied()
        .cycle()
        .take((sheet_width * sheet_height * 3) as usize)
        .collect();
    let mut blit = |flag: &Flag, left: u32, top: u32| {
        let pixels = flag.paint(WIDTH, HEIGHT);
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let from = ((y * WIDTH + x) * 4) as usize;
                let to = (((top + y) * sheet_width + left + x) * 3) as usize;
                rgb[to..to + 3].copy_from_slice(&pixels[from..from + 3]);
            }
        }
    };
    for row in 0..rows {
        let seed = first + u64::from(row);
        let top = GUTTER + row * (HEIGHT + GUTTER);
        let ours = Flag::generate(seed);
        blit(&ours, GUTTER, top);
        println!("{seed:>5}  {ours}");
        for leg in 0..RIVALS {
            let theirs = Flag::rival(seed.wrapping_mul(0x9E37_79B9) ^ u64::from(leg), &ours);
            blit(&theirs, GUTTER * 3 + (leg + 1) * (WIDTH + GUTTER), top);
            println!(
                "       vs {theirs}  ({:.0}% different)",
                100. * theirs.difference(&ours)
            );
        }
    }
    std::fs::create_dir_all("work/flags").expect("create work/flags");
    let path = "work/flags/sheet.png";
    let file = std::fs::File::create(path).expect("create the sheet");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), sheet_width, sheet_height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(&rgb))
        .expect("write the sheet");
    println!("\nwrote {path}");
}
