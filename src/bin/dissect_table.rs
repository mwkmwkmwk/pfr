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
    save_png(&assets.main_board, &args.output_dir, "main.png")?;
    save_png(&assets.spring, &args.output_dir, "spring.png")?;
    Ok(())
}
