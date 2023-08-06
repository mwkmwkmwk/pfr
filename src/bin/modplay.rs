use clap::Parser;
use std::{fs::File, path::PathBuf};

#[derive(Parser)]
struct Args {
    modfile: PathBuf,
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();
    let mut f = File::open(args.modfile)?;
    let module = pfr::sound::loader::load(&mut f)?;
    let player = pfr::sound::player::play(module, true);
    // println!("NAME: {}", module.name);
    // for (i, pat) in module.patterns.iter().enumerate() {
    //     println!("--- PAT {i:02x} ---");
    //     for (j, l) in pat.iter().enumerate() {
    //         print!("{j:02x}:");
    //         for n in l {
    //             print!("   {n}");
    //         }
    //         println!();
    //     }
    // }
    let stdin = std::io::stdin();
    loop {
        let mut buf = String::new();
        stdin.read_line(&mut buf)?;

        let c = buf.trim();
        if let Some(r) = c.strip_prefix('j') {
            let Ok(r) = u32::from_str_radix(r, 16) else {
                continue;
            };
            player.jingle(
                (r & 0xff) as u8,
                (r >> 8 & 0xf) as u8,
                (r >> 12 & 0x3f) as u8,
                false,
            );
        }
        if let Some(r) = c.strip_prefix('m') {
            let Ok(r) = u32::from_str_radix(r, 16) else {
                continue;
            };
            player.set_music_pos((r & 0xff) as u8);
        }
        if let Some(r) = c.strip_prefix('s') {
            let Ok(r) = u32::from_str_radix(r, 16) else {
                continue;
            };
            player.play_sfx(
                (r & 0xff) as u8,
                (r >> 8 & 0xff) as u8,
                (r >> 16 & 0xff) as u8,
                (r >> 24 & 0xff) as u8,
            );
        }
    }
}
