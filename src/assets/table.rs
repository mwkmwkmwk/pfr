use std::path::Path;

use ndarray::{concatenate, prelude::*};

use crate::config::TableId;

use super::{iff::Image, mz::MzExe};

#[derive(Clone, Debug)]
pub struct Assets {
    pub table: TableId,
    pub exe: MzExe,
    pub main_board: Image,
    pub spring: Image,
}

fn extract_main_board(exe: &MzExe, table: TableId) -> Image {
    let pbm_segs = match table {
        TableId::Table1 => [0x5224, 0x5947, 0x617b, 0x6a9c],
        TableId::Table2 => todo!(),
        TableId::Table3 => todo!(),
        TableId::Table4 => [0x4ba1, 0x5480, 0x5d87, 0x66c2],
    };
    let pbms = pbm_segs.map(|x| Image::parse(exe.segment(x)));
    Image {
        data: concatenate!(
            Axis(1),
            pbms[0].data.slice(s![.., ..144]),
            pbms[1].data.slice(s![.., ..144]),
            pbms[2].data.slice(s![.., ..144]),
            pbms[3].data.slice(s![.., ..144]),
        ),
        cmap: pbms[3].cmap.clone(),
    }
}

fn extract_spring(exe: &MzExe, table: TableId) -> Image {
    let spring_seg = match table {
        TableId::Table1 => 0x82e2,
        TableId::Table2 => todo!(),
        TableId::Table3 => todo!(),
        TableId::Table4 => 0x7f4f,
    };
    let spring = exe.segment(spring_seg);
    Image {
        data: Array2::from_shape_fn((10, 23), |(x, y)| spring[y * 10 + x]),
        cmap: vec![],
    }
}

impl Assets {
    pub fn load(file: impl AsRef<Path>, table: TableId) -> std::io::Result<Self> {
        let mut exe = MzExe::load(file, 0)?;
        assert_eq!(exe.code_byte(exe.ip + 0xe), 0xb8);
        let ds = exe.code_word(exe.ip + 0xf);
        exe.ds = ds;

        let main_board = extract_main_board(&exe, table);
        let mut spring = extract_spring(&exe, table);
        spring.cmap = main_board.cmap.clone();

        Ok(Assets {
            table,
            exe,
            main_board,
            spring,
        })
    }
}
