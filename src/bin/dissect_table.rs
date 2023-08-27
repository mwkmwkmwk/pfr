use clap::Parser;
use pfr::assets::iff::Image;
use pfr::assets::table::Assets;
use pfr::config::TableId;
use std::io::BufWriter;
use std::{
    fs::File,
    path::{Path, PathBuf},
};

#[derive(Parser)]
struct Args {
    input_dir: PathBuf,
    table: u32,
    output_dir: PathBuf,
}

fn save_png(image: &Image, output_dir: impl AsRef<Path>, name: &str) -> std::io::Result<()> {
    let width = image.data.dim().0;
    let height = image.data.dim().1;
    let file = File::create(output_dir.as_ref().join(name))?;
    let w = BufWriter::new(file);
    let mut encoder = png::Encoder::new(w, width as u32, height as u32);
    encoder.set_color(png::ColorType::Indexed);
    encoder.set_depth(png::BitDepth::Eight);
    let mut cmap = vec![0; image.cmap.len() * 3];
    for i in 0..image.cmap.len() {
        cmap[i * 3] = image.cmap[i].0;
        cmap[i * 3 + 1] = image.cmap[i].1;
        cmap[i * 3 + 2] = image.cmap[i].2;
    }
    encoder.set_palette(&cmap);
    let mut writer = encoder.write_header()?;
    let mut data = vec![0; width * height];
    for y in 0..height {
        for x in 0..width {
            data[y * width + x] = image.data[(x, y)];
        }
    }
    writer.write_image_data(&data)?;
    writer.finish()?;
    Ok(())
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();
    let (table, file) = match args.table {
        1 => (TableId::Table1, "TABLE1.PRG"),
        2 => (TableId::Table2, "TABLE2.PRG"),
        3 => (TableId::Table3, "TABLE3.PRG"),
        4 => (TableId::Table4, "TABLE4.PRG"),
        _ => panic!("oops weird table"),
    };
    let assets = Assets::load(args.input_dir.join(file), table)?;
    println!("DS: {ds:04x}", ds = assets.exe.ds);
    let mut main_board = assets.main_board.clone();

    for patch in &assets.pal_patches {
        for (i, &color) in patch.colors.iter().enumerate() {
            main_board.cmap[patch.base_index as usize + i] = color
        }
    }

    save_png(&main_board, &args.output_dir, "main.png")?;

    save_png(
        &Image {
            data: assets.occmaps[0].clone(),
            cmap: vec![(0, 0, 0), (255, 255, 255)],
        },
        &args.output_dir,
        "occmap0.png",
    )?;
    save_png(
        &Image {
            data: assets.occmaps[1].clone(),
            cmap: vec![(0, 0, 0), (255, 255, 255)],
        },
        &args.output_dir,
        "occmap1.png",
    )?;
    save_png(&assets.spring, &args.output_dir, "spring.png")?;
    save_png(&assets.ball, &args.output_dir, "ball.png")?;
    let physmap_pal = vec![
        (0, 0, 0),
        (0, 0, 64),
        (0, 64, 0),
        (64, 0, 0),
        (64, 64, 0),
        (64, 0, 64),
        (0, 64, 64),
        (64, 64, 64),
        (64, 0, 32),
        (64, 32, 0),
        (0, 64, 32),
        (32, 64, 0),
        (0, 32, 64),
        (32, 0, 64),
        (32, 32, 0),
        (0, 32, 32),
        (128, 128, 128),
        (128, 128, 255),
        (128, 255, 128),
        (128, 255, 255),
        (255, 128, 128),
        (255, 128, 255),
        (255, 255, 128),
        (255, 255, 255),
    ];
    save_png(
        &Image {
            data: assets.physmaps[0].clone(),
            cmap: physmap_pal.clone(),
        },
        &args.output_dir,
        "physmap0.png",
    )?;
    save_png(
        &Image {
            data: assets.physmaps[1].clone(),
            cmap: physmap_pal.clone(),
        },
        &args.output_dir,
        "physmap1.png",
    )?;

    Ok(())
}
