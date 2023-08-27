use std::{fs::File, path::Path, ptr};

use kvm_bindings::kvm_userspace_memory_region;
use kvm_ioctls::Kvm;
use winit::event::{ElementState, VirtualKeyCode};

use crate::{
    assets::table::Assets,
    config::{Config, TableId},
    sound::player::Player,
    view::{Action, Route, View},
};

pub struct Table {
    player: Player,
    assets: Assets,
    config: Config,
    ivt_addr: *mut u8,
    exe_addr: *mut u8,
}

impl Table {
    pub fn new(data: &Path, config: Config, table: TableId) -> Table {
        let (prg, module) = match table {
            TableId::Table1 => ("TABLE1.PRG", "TABLE1.MOD"),
            TableId::Table2 => ("TABLE2.PRG", "TABLE2.MOD"),
            TableId::Table3 => ("TABLE3.PRG", "TABLE3.MOD"),
            TableId::Table4 => ("TABLE4.PRG", "TABLE4.MOD"),
        };
        let mut f = File::open(data.join(module)).unwrap();
        let module = crate::sound::loader::load(&mut f).unwrap();
        let player = crate::sound::player::play(module, false);
        let assets = Assets::load(data.join(prg), table).unwrap();

        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();

        let ivt_addr: *mut u8 = unsafe {
            libc::mmap(
                ptr::null_mut(),
                0x1000,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_ANONYMOUS | libc::MAP_SHARED,
                -1,
                0,
            ) as *mut u8
        };
        let mem_region = kvm_userspace_memory_region {
            slot: 0,
            guest_phys_addr: 0,
            memory_size: 0x1000,
            userspace_addr: ivt_addr as u64,
            flags: 0,
        };
        unsafe { vm.set_user_memory_region(mem_region).unwrap() };

        unsafe {
            let slice = std::slice::from_raw_parts_mut(ivt_addr as *mut u32, 0x100);
            for i in 0..0x100 {
                slice[i] = (0x800 + i * 8) as u32;
            }
        }

        let exe_addr: *mut u8 = unsafe {
            libc::mmap(
                ptr::null_mut(),
                assets.exe.image.len(),
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_ANONYMOUS | libc::MAP_SHARED,
                -1,
                0,
            ) as *mut u8
        };
        let mem_region = kvm_userspace_memory_region {
            slot: 1,
            guest_phys_addr: 0x2000,
            memory_size: ((assets.exe.image.len() + 0xfff) & !0xfff) as u64,
            userspace_addr: exe_addr as u64,
            flags: 0,
        };
        unsafe { vm.set_user_memory_region(mem_region).unwrap() };

        unsafe {
            let slice = std::slice::from_raw_parts_mut(exe_addr, assets.exe.image.len());
            slice.copy_from_slice(&assets.exe.image);
            for &ptr in &assets.exe.relocs {
                let ptr = exe_addr.offset(ptr.off as isize + ptr.seg as isize * 0x10);
                let ptr = ptr as *mut u16;
                let cur = ptr.read_unaligned();
                ptr.write_unaligned(cur + 0x200);
            }
        }

        let vcpu_fd = vm.create_vcpu(0).unwrap();

        let mut vcpu_sregs = vcpu_fd.get_sregs().unwrap();
        vcpu_sregs.cs.base = (assets.exe.cs + 0x200) as u64 * 0x10;
        vcpu_sregs.cs.selector = assets.exe.cs + 0x200;
        vcpu_sregs.ss.base = (assets.exe.ss + 0x200) as u64 * 0x10;
        vcpu_sregs.ss.selector = assets.exe.ss + 0x200;
        let init_ds = 0x1f0;
        vcpu_sregs.ds.base = init_ds as u64 * 0x10;
        vcpu_sregs.ds.selector = init_ds;
        vcpu_sregs.es.base = init_ds as u64 * 0x10;
        vcpu_sregs.es.selector = init_ds;
        vcpu_fd.set_sregs(&vcpu_sregs).unwrap();

        let mut vcpu_regs = vcpu_fd.get_regs().unwrap();
        vcpu_regs.rip = assets.exe.ip as u64;
        vcpu_regs.rsp = assets.exe.sp as u64;
        vcpu_fd.set_regs(&vcpu_regs).unwrap();

        let out = vcpu_fd.run().unwrap();
        println!("OUT: {out:?}");
        let vcpu_regs = vcpu_fd.get_regs().unwrap();
        println!("REGS: {vcpu_regs:x?}");

        Table {
            player,
            assets,
            config,
            ivt_addr,
            exe_addr,
        }
    }
}

impl View for Table {
    fn get_resolution(&self) -> (u32, u32) {
        (320, 240)
    }

    fn get_fps(&self) -> u32 {
        60
    }

    fn run_frame(&mut self) -> Action {
        // todo!()
        Action::Navigate(Route::Intro(Some(self.assets.table)))
    }

    fn handle_key(&mut self, key: VirtualKeyCode, state: ElementState) {
        // todo!()
    }

    fn render(&self, data: &mut [u8], pal: &mut [(u8, u8, u8)]) {
        // todo!()
    }
}
