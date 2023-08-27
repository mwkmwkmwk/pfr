use std::path::Path;

use ndarray::{concatenate, prelude::*};

use crate::config::TableId;

use super::{iff::Image, mz::MzExe};

#[derive(Clone, Debug)]
pub struct Assets {
    pub table: TableId,
    pub exe: MzExe,
    pub main_board: Image,
    pub occmaps: [Array2<u8>; 2],
    pub spring: Image,
    pub ball: Image,
    pub physmaps: [Array2<u8>; 2],
}

fn extract_main_board(exe: &MzExe, table: TableId) -> Image {
    let pbm_segs = match table {
        TableId::Table1 => [0x5224, 0x5947, 0x617b, 0x6a9c],
        TableId::Table2 => [0x5054, 0x5820, 0x5fe4, 0x6791],
        TableId::Table3 => [0x4c96, 0x5221, 0x5a4b, 0x632d],
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

fn extract_occmaps(exe: &MzExe, table: TableId) -> [Array2<u8>; 2] {
    let seg = match table {
        TableId::Table1 => 0x2f94,
        TableId::Table2 => 0x2cd8,
        TableId::Table3 => 0x1f0f,
        TableId::Table4 => 0x287b,
    };
    [0x580, 0x6400].map(|off| {
        Array2::from_shape_fn((320, 576), |(x, y)| {
            let byte = exe.byte(seg, off + (x / 8 + y * 40) as u16);
            byte >> (7 - x % 8) & 1
        })
    })
}

fn extract_physmaps(exe: &MzExe, table: TableId) -> [Array2<u8>; 2] {
    let segs = match table {
        TableId::Table1 => todo!(),
        TableId::Table2 => todo!(),
        TableId::Table3 => todo!(),
        TableId::Table4 => [(0x39fb, 0x345b, 0x6e67), (0x7407, 0x3f9b, 0x79a7)],
    };
    segs.map(|(s0, s1, s2)| {
        Array2::from_shape_fn((320, 576), |(x, y)| {
            let off = (x / 8 + y * 40) as u16;
            let b0 = exe.byte(s0, off);
            let b1 = exe.byte(s1, off);
            let b2 = exe.byte(s2, off);
            let bit0 = b0 >> (7 - x % 8) & 1;
            let bit1 = b1 >> (7 - x % 8) & 1;
            let bit2 = b2 >> (7 - x % 8) & 1;
            let mut val = 0x10 | bit2 << 2 | bit1 << 1 | bit0;
            if val & 3 == 0 {
                if off != 0 && exe.byte(s0, off - 1) == 0 && exe.byte(s1, off - 1) == 0 {
                    val = exe.byte(s2, off - 1) & 0xf;
                } else if exe.byte(s0, off) == 0 && exe.byte(s1, off) == 0 {
                    val = exe.byte(s2, off) & 0xf;
                } else if exe.byte(s0, off + 1) == 0 && exe.byte(s1, off + 1) == 0 {
                    val = exe.byte(s2, off + 1) & 0xf;
                }
            }
            val
        })
    })
}

fn extract_spring(exe: &MzExe, table: TableId) -> Array2<u8> {
    let spring_seg = match table {
        TableId::Table1 => 0x82e2,
        TableId::Table2 => 0x7e48,
        TableId::Table3 => 0x7b0d,
        TableId::Table4 => 0x7f4f,
    };
    let spring = exe.segment(spring_seg);
    Array2::from_shape_fn((10, 23), |(x, y)| spring[y * 10 + x])
}

fn extract_ball(exe: &MzExe, table: TableId) -> Array2<u8> {
    let base = match table {
        TableId::Table1 => 0x95b0,
        TableId::Table2 => 0x8da0,
        TableId::Table3 => 0x8830,
        TableId::Table4 => 0x9d40,
    };
    let mut res = Array2::zeros((15, 15));
    let mut pos = base + 0x57;
    let mut plane = 0;
    let mut bbit = 0;
    loop {
        match exe.code_byte(pos) {
            0x26 => {
                assert_eq!(exe.code_byte(pos), 0x26);
                assert_eq!(exe.code_byte(pos + 1), 0x84);
                let boff = match exe.code_byte(pos + 2) {
                    0x27 => {
                        pos += 3;
                        0
                    }
                    0x67 => {
                        let x = exe.code_byte(pos + 3);
                        assert!(x < 0x80);
                        pos += 4;
                        x as u16
                    }
                    0xa7 => {
                        let x = exe.code_word(pos + 3);
                        pos += 5;
                        x
                    }
                    _ => unreachable!(),
                };
                assert_eq!(exe.code_byte(pos), 0x75);
                let jd = exe.code_byte(pos + 1);
                assert!(jd < 0x80);
                pos += 2;
                let jdst = pos + (jd as u16);
                assert_eq!(exe.code_byte(pos), 0x8a);
                let poff = match exe.code_byte(pos + 1) {
                    0x44 => {
                        let x = exe.code_byte(pos + 2);
                        assert!(x < 0x80);
                        pos += 3;
                        x as u16
                    }
                    0x84 => {
                        let x = exe.code_word(pos + 2);
                        pos += 4;
                        x
                    }
                    _ => unreachable!(),
                };
                assert_eq!(exe.code_byte(pos), 0xaa);
                pos += 1;
                assert_eq!(exe.code_byte(pos), 0xc6);
                let poff2 = match exe.code_byte(pos + 1) {
                    0x44 => {
                        let x = exe.code_byte(pos + 2);
                        assert!(x < 0x80);
                        pos += 3;
                        x as u16
                    }
                    0x84 => {
                        let x = exe.code_word(pos + 2);
                        pos += 4;
                        x
                    }
                    _ => unreachable!(),
                };
                let pix = exe.code_byte(pos);
                pos += 1;
                let py = poff / 84;
                let px = poff % 84 * 4 + plane;
                assert_eq!(poff, poff2);
                assert_eq!(bbit, px % 8);
                assert_eq!(boff, px / 8 + py * 42);
                res[(px as usize, py as usize)] = pix;
                assert_eq!(jdst, pos);
            }
            0xd0 if exe.code_byte(pos + 1) == 0xcc => {
                for _ in 0..4 {
                    assert_eq!(exe.code_bytes(pos, 5), [0xd0, 0xcc, 0x73, 0x01, 0x43]);
                    pos += 5;
                }
                bbit += 4;
                bbit %= 8;
            }
            0xd0 if exe.code_byte(pos + 1) == 0xc1 => {
                assert_eq!(
                    exe.code_bytes(pos, 0x2e),
                    [
                        0xd0, 0xc1, 0x83, 0xd6, 0x00, 0xfe, 0xc5, 0x80, 0xe5, 0x03, 0x50, 0x8a,
                        0xe5, 0xb0, 0x04, 0xba, 0xce, 0x03, 0xef, 0xba, 0xc4, 0x03, 0xb0, 0x02,
                        0x8a, 0xe1, 0x80, 0xe4, 0x0f, 0xef, 0x58, 0xd0, 0xc4, 0x73, 0x01, 0x4b,
                        0xd0, 0xc4, 0x73, 0x01, 0x4b, 0xd0, 0xc4, 0x73, 0x01, 0x4b,
                    ]
                );
                pos += 0x2e;
                plane += 1;
                bbit += 5;
                bbit %= 8;
            }
            0x5a => {
                assert_eq!(exe.code_bytes(pos, 3), [0x5a, 0x5e, 0xc3]);
                break;
            }
            x => panic!("ummm {x:02x} at {pos:04x}"),
        }
    }
    res
}

impl Assets {
    pub fn load(file: impl AsRef<Path>, table: TableId) -> std::io::Result<Self> {
        let mut exe = MzExe::load(file, 0)?;
        assert_eq!(exe.code_byte(exe.ip + 0xe), 0xb8);
        let ds = exe.code_word(exe.ip + 0xf);
        exe.ds = ds;

        let main_board = extract_main_board(&exe, table);
        let occmaps = extract_occmaps(&exe, table);
        let spring = Image {
            data: extract_spring(&exe, table),
            cmap: main_board.cmap.clone(),
        };
        let ball = Image {
            data: extract_ball(&exe, table),
            cmap: main_board.cmap.clone(),
        };

        let physmaps = extract_physmaps(&exe, table);

        Ok(Assets {
            table,
            exe,
            main_board,
            occmaps,
            spring,
            ball,
            physmaps,
        })
    }
}
