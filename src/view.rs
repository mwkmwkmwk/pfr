use winit::event::{ElementState, VirtualKeyCode};

use crate::config::TableId;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Route {
    Intro(Option<TableId>),
    Table(TableId),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Action {
    None,
    Navigate(Route),
    Exit,
}

pub trait View {
    fn get_resolution(&self) -> (u32, u32);
    fn get_fps(&self) -> u32;
    fn run_frame(&mut self) -> Action;
    fn handle_key(&mut self, key: VirtualKeyCode, state: ElementState);
    fn render(&self, data: &mut [u8], pal: &mut [(u8, u8, u8)]);
}
