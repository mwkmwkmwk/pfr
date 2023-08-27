use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    BufferSize, SampleRate, Stream, StreamConfig,
};

use super::{MiscEffect, Mod, Note, ToneEffect, VolumeEffect, PERIODS};

const VIBRATO_LUT: [u8; 32] = [
    0x00, 0x18, 0x31, 0x4a, 0x61, 0x78, 0x8d, 0xa1, 0xb4, 0xc5, 0xd4, 0xe0, 0xeb, 0xf4, 0xfa, 0xfd,
    0xff, 0xfd, 0xfa, 0xf4, 0xeb, 0xe0, 0xd4, 0xc5, 0xb4, 0xa1, 0x8d, 0x78, 0x61, 0x4a, 0x31, 0x18,
];

struct PlayerState {
    module: Mod,
    control: Arc<PlayerControl>,
    sample_rate: u32,
    speed: u8,
    ticks_left: u8,
    samples_left: u32,
    samples_in_tick: u32,
    position: usize,
    row: usize,
    started: bool,
    channels: [ChannelState; 4],
    pattern_break: Option<u8>,
    jump: Option<u8>,
}

enum ChannelToneEffect {
    None,
    Portamento,
    Vibrato,
    Arpeggio,
    Retrig,
}

enum ChannelVolumeEffect {
    None,
    Slide,
}

struct ChannelState {
    volume: u8,
    sample: usize,
    sample_pos: u64,
    sample_bytes_per_frame: u64,
    sample_pos_reload: u64,
    xperiod: u8,
    period: u16,
    tone_effect: ChannelToneEffect,
    arpeggio_periods: [u16; 2],
    portamento_target: u16,
    portamento_speed: u8,
    vibrato_phase: u8,
    vibrato_rate: u8,
    vibrato_depth: u8,
    volume_effect: ChannelVolumeEffect,
    volume_slide_speed: i8,
    retrig_period: u8,
    retrig_left: u8,
}

struct PlayerControl {
    cmd: AtomicU32,
    ticks: AtomicU32,
    sfx: AtomicU32,
    state: AtomicU32,
}

impl PlayerControl {
    const CMD_JUMP_POSITION: u32 = 0x7f;
    const CMD_JUMP_VALID: u32 = 0x80;
    const STATE_PAUSED: u32 = 0x200;
    const STATE_MASTER_VOLUME: u32 = 0x1ff;
}

pub struct Player {
    _stream: Stream,
    control: Arc<PlayerControl>,
}

impl Player {
    pub fn jingle(&self, pos: u8, repeat: u8, prio: u8, hard: bool) {
        assert!(repeat < 0x10);
        assert!(prio < 0x40);
        let mut val = self.control.cmd.load(Ordering::Acquire);
        loop {
            let cur_prio = val >> 12 & 0x3f;
            if (prio as u32) < cur_prio && !hard {
                return;
            }
            let new_val =
                val & !0x3ffff | 0x80 | (pos as u32) | (repeat as u32) << 8 | (prio as u32) << 12;
            match self.control.cmd.compare_exchange(
                val,
                new_val,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(x) => val = x,
            }
        }
    }

    pub fn set_music_pos(&self, pos: u8) {
        let mut val = self.control.cmd.load(Ordering::Acquire);
        loop {
            let new_val = val & !0x7f000000 | (pos as u32) << 24;
            match self.control.cmd.compare_exchange(
                val,
                new_val,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(x) => val = x,
            }
        }
    }

    pub fn set_music_prio(&self, prio: u8) {
        let mut val = self.control.cmd.load(Ordering::Acquire);
        loop {
            let new_val = val & !0x00fc0000 | (prio as u32) << 18;
            match self.control.cmd.compare_exchange(
                val,
                new_val,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(x) => val = x,
            }
        }
    }

    pub fn play_sfx(&self, period: u8, sample: u8, volume: u8, channel: u8) {
        let val =
            (period as u32) | (sample as u32) << 8 | (volume as u32) << 16 | (channel as u32) << 24;
        self.control.sfx.store(val, Ordering::Relaxed);
    }

    pub fn get_ticks(&self) -> u32 {
        self.control.ticks.load(Ordering::Acquire)
    }

    pub fn set_master_volume(&self, volume: u32) {
        assert!(volume <= 0x100);
        self.control.state.store(volume, Ordering::Relaxed);
    }
}

pub fn play(module: Mod, start: bool) -> Player {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("no output device available");
    /*let supported_configs_range = device
        .supported_output_configs()
        .expect("error while querying configs");
    for cfg in supported_configs_range {
        println!("{cfg:#?}");
    }*/
    let sample_rate = 48000;
    let control = Arc::new(PlayerControl {
        cmd: AtomicU32::new(0),
        ticks: AtomicU32::new(0),
        sfx: AtomicU32::new(0),
        state: AtomicU32::new(0x100),
    });
    let mut state = PlayerState {
        module,
        speed: 6,
        ticks_left: 0,
        samples_left: 0,
        control: control.clone(),
        samples_in_tick: sample_rate / 50,
        position: 0,
        row: 0,
        started: start,
        channels: std::array::from_fn(|_| ChannelState {
            volume: 0x40,
            sample: 0,
            sample_pos: 0,
            sample_bytes_per_frame: 0,
            sample_pos_reload: 0,
            period: 0,
            vibrato_phase: 0,
            tone_effect: ChannelToneEffect::None,
            arpeggio_periods: [0, 0],
            portamento_target: 0,
            portamento_speed: 0,
            vibrato_rate: 0,
            vibrato_depth: 0,
            volume_effect: ChannelVolumeEffect::None,
            volume_slide_speed: 0,
            retrig_period: 0,
            retrig_left: 0,
            xperiod: 0,
        }),
        sample_rate,
        pattern_break: None,
        jump: None,
    };
    let config = StreamConfig {
        channels: 2,
        sample_rate: SampleRate(sample_rate),
        buffer_size: BufferSize::Fixed(sample_rate / 50),
    };
    let stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| state.make_samples(data),
            move |err| eprintln!("audio error: {err:?}"),
            None, // None=blocking, Some(Duration)=timeout
        )
        .expect("failed to make stream");
    stream.play().unwrap();
    Player {
        _stream: stream,
        control,
    }
}

impl PlayerState {
    fn make_samples(&mut self, data: &mut [f32]) {
        let state = self.control.state.load(Ordering::Relaxed);
        if (state & PlayerControl::STATE_PAUSED) != 0 {
            for v in data {
                *v = 0.0;
            }
            return;
        }
        let master_volume = (state & PlayerControl::STATE_MASTER_VOLUME) as i32;
        self.process_cmd();
        if !self.started {
            for v in data {
                *v = 0.0;
            }
            return;
        }
        let sfx = self.control.sfx.swap(0, Ordering::Relaxed);
        if sfx != 0 {
            let volume = (sfx >> 16 & 0xff) as u8;
            let channel = (sfx >> 24 & 0xff) as usize;
            let note = Note {
                period: Some((sfx & 0xff) as u8),
                sample: Some((sfx >> 8 & 0xff) as u8),
                tone_effect: ToneEffect::None,
                volume_effect: if volume == 0 {
                    VolumeEffect::None
                } else {
                    VolumeEffect::SetVolume(volume)
                },
                misc_effect: MiscEffect::None,
            };
            self.play_note(channel, note);
        }
        let mut pos = 0;
        while pos < data.len() {
            if self.samples_left == 0 {
                if self.ticks_left == 0 {
                    self.play_row();
                    self.ticks_left = self.speed - 1;
                } else {
                    self.ticks_left -= 1;
                    self.play_effects();
                }
                self.samples_left = self.samples_in_tick;
                let ticks = self.control.ticks.load(Ordering::Relaxed);
                self.control.ticks.store(ticks + 1, Ordering::Release);
            }
            data[pos] = ((self.play_channel(0) + self.play_channel(1)) / 0x100 * master_volume)
                as f32
                / (0x80000000u32 as f32);
            data[pos + 1] = ((self.play_channel(2) + self.play_channel(3)) / 0x100 * master_volume)
                as f32
                / (0x80000000u32 as f32);
            pos += 2;
            self.samples_left -= 1;
        }
    }

    fn process_cmd(&mut self) {
        let mut cmd = self.control.cmd.load(Ordering::Acquire);
        loop {
            if cmd & PlayerControl::CMD_JUMP_VALID == 0 {
                return;
            }
            let new_cmd = cmd & !PlayerControl::CMD_JUMP_VALID;
            match self.control.cmd.compare_exchange(
                cmd,
                new_cmd,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(val) => cmd = val,
            }
        }
        self.position = (cmd & PlayerControl::CMD_JUMP_POSITION) as usize;
        self.row = 0;
        self.ticks_left = 0;
        self.samples_left = 0;
        self.started = true;
    }

    fn jump(&mut self, mut pos: u8) {
        let mut val = self.control.cmd.load(Ordering::Acquire);
        loop {
            if val & 0x80 != 0 {
                // doesn't matter, the command will override everything anyway.
                break;
            }
            let repeat = val >> 8 & 0xf;
            match repeat {
                0 => {
                    // nothing to worry about, just jump
                    break;
                }
                1 => {
                    // repeat ran out, jump to music instead
                    let music_prio = val >> 18 & 0x3f;
                    let new_val = val & !0x3ff00 | music_prio << 12;
                    pos = (val >> 24) as u8;
                    match self.control.cmd.compare_exchange(
                        val,
                        new_val,
                        Ordering::Release,
                        Ordering::Acquire,
                    ) {
                        Ok(_) => break,
                        Err(x) => val = x,
                    }
                }
                _ => {
                    // decrease repeat count
                    let new_val = val & !0xf00 | (repeat - 1) << 8;
                    match self.control.cmd.compare_exchange(
                        val,
                        new_val,
                        Ordering::Release,
                        Ordering::Acquire,
                    ) {
                        Ok(_) => break,
                        Err(x) => val = x,
                    }
                }
            }
        }
        self.jump = Some(pos)
    }

    fn play_row(&mut self) {
        let pattern = self.module.positions[self.position] as usize;
        let row = self.module.patterns[pattern][self.row];
        print!(
            "{pos:02x}/{pattern:02x}.{r:02x}",
            pos = self.position,
            r = self.row
        );
        for (i, &note) in row.iter().enumerate() {
            self.play_note(i, note);
            print!("   {note}");
        }
        println!();
        if let Some(pos) = self.jump {
            println!("---JUMP---");
            self.position = pos as usize;
            self.row = 0;
            self.jump = None;
        } else if let Some(row) = self.pattern_break {
            println!("---BREAK---");
            self.row = row as usize;
            self.position += 1;
            if self.position == self.module.positions.len() {
                self.position = 0;
            }
            self.pattern_break = None;
        } else {
            self.row += 1;
            if self.row == 0x40 {
                println!("---");
                self.row = 0;
                self.position += 1;
                if self.position == self.module.positions.len() {
                    self.position = 0;
                }
            }
        }
    }

    fn play_note(&mut self, cidx: usize, note: Note) {
        let channel = &mut self.channels[cidx];
        if let Some(sidx) = note.sample {
            channel.sample = sidx as usize;
            channel.sample_pos_reload = 0;
            let sample = &self.module.samples[channel.sample];
            channel.volume = sample.volume;
        }
        let sample = &self.module.samples[channel.sample];
        if let Some(xperiod) = note.period {
            let period = PERIODS[sample.finetune as usize][xperiod as usize];
            channel.xperiod = xperiod;
            channel.period = period;
            channel.sample_pos = channel.sample_pos_reload;
            channel.vibrato_phase = 0;
            let byte_len = 0x361f0f / (period as u32);
            channel.sample_bytes_per_frame = ((byte_len as u64) << 32) / (self.sample_rate as u64);
        }
        match note.tone_effect {
            super::ToneEffect::None => channel.tone_effect = ChannelToneEffect::None,
            super::ToneEffect::Arpeggio(a, b) => {
                channel.tone_effect = ChannelToneEffect::Arpeggio;
                channel.arpeggio_periods = [
                    PERIODS[sample.finetune as usize][(channel.xperiod + a).min(35) as usize],
                    PERIODS[sample.finetune as usize][(channel.xperiod + b).min(35) as usize],
                ];
            }
            super::ToneEffect::Portamento { target, speed } => {
                channel.tone_effect = ChannelToneEffect::Portamento;
                if let Some(v) = target {
                    channel.portamento_target = PERIODS[sample.finetune as usize][v as usize];
                }
                if let Some(v) = speed {
                    channel.portamento_speed = v.into();
                }
            }
            super::ToneEffect::Vibrato { rate, depth } => {
                channel.tone_effect = ChannelToneEffect::Vibrato;
                if let Some(v) = rate {
                    channel.vibrato_rate = v.get() * 4;
                }
                if let Some(v) = depth {
                    channel.vibrato_depth = v.get();
                }
            }
        }
        match note.volume_effect {
            super::VolumeEffect::None => channel.volume_effect = ChannelVolumeEffect::None,
            super::VolumeEffect::SetVolume(v) => {
                channel.volume_effect = ChannelVolumeEffect::None;
                channel.volume = v;
            }
            super::VolumeEffect::VolumeSlide(s) => {
                channel.volume_effect = ChannelVolumeEffect::Slide;
                channel.volume_slide_speed = s;
            }
            super::VolumeEffect::Reset => {
                channel.volume_effect = ChannelVolumeEffect::None;
                channel.volume = sample.volume;
            }
        }
        match note.misc_effect {
            MiscEffect::None => {}
            MiscEffect::SetSampleOffset(off) => {
                channel.sample_pos_reload = (off as u64) << 40;
                if note.sample.is_some() {
                    channel.sample_pos = channel.sample_pos_reload;
                }
            }
            MiscEffect::PositionJump(pos) => self.jump(pos),
            MiscEffect::PatternBreak(x) => {
                self.pattern_break = Some(x);
            }
            MiscEffect::RetrigNote(x) => {
                channel.tone_effect = ChannelToneEffect::Retrig;
                channel.retrig_period = x;
                channel.retrig_left = x - 1;
            }
            MiscEffect::SetSpeed(s) => {
                self.speed = s;
                self.ticks_left = s - 1;
            }
        }
    }

    fn play_effects(&mut self) {
        for channel in &mut self.channels {
            match channel.tone_effect {
                ChannelToneEffect::None => {}
                ChannelToneEffect::Arpeggio => {
                    let tmp = channel.period;
                    channel.period = channel.arpeggio_periods[1];
                    channel.arpeggio_periods[1] = channel.arpeggio_periods[0];
                    channel.arpeggio_periods[0] = tmp;
                    let byte_len = 0x361f0f / (channel.period as u32);
                    channel.sample_bytes_per_frame =
                        ((byte_len as u64) << 32) / (self.sample_rate as u64);
                }
                ChannelToneEffect::Portamento => {
                    // println!("PORTAMENTO!");
                    if channel.portamento_target != 0 {
                        if channel.portamento_target < channel.period {
                            channel.period -= channel.portamento_speed as u16;
                            if channel.period < channel.portamento_target {
                                channel.period = channel.portamento_target;
                            }
                        } else {
                            channel.period += channel.portamento_speed as u16;
                            if channel.period > channel.portamento_target {
                                channel.period = channel.portamento_target;
                            }
                        }
                        let byte_len = 0x361f0f / (channel.period as u32);
                        channel.sample_bytes_per_frame =
                            ((byte_len as u64) << 32) / (self.sample_rate as u64);
                    }
                }
                ChannelToneEffect::Vibrato => {
                    let phase = channel.vibrato_phase;
                    channel.vibrato_phase = phase.wrapping_add(channel.vibrato_rate);
                    let mut delta = VIBRATO_LUT[(phase >> 2 & 0x1f) as usize] as i16;
                    delta *= channel.vibrato_depth as i16;
                    delta >>= 7;
                    if phase & 0x80 != 0 {
                        delta *= -1;
                    }
                    // println!("VIBRATO {delta}");
                    let period = channel.period.wrapping_add_signed(delta);
                    let byte_len = 0x361f0f / (period as u32);
                    channel.sample_bytes_per_frame =
                        ((byte_len as u64) << 32) / (self.sample_rate as u64);
                }
                ChannelToneEffect::Retrig => {
                    if channel.retrig_left == 0 {
                        channel.retrig_left = channel.retrig_period - 1;
                        channel.sample_pos = 0;
                    } else {
                        channel.retrig_left -= 1;
                    }
                }
            }
            match channel.volume_effect {
                ChannelVolumeEffect::None => {}
                ChannelVolumeEffect::Slide => {
                    channel.volume = channel
                        .volume
                        .saturating_add_signed(channel.volume_slide_speed);
                    if channel.volume > 0x40 {
                        channel.volume = 0x40;
                    }
                }
            }
        }
    }

    fn play_channel(&mut self, idx: usize) -> i32 {
        let channel = &mut self.channels[idx];
        let sample = &self.module.samples[channel.sample];
        let mut pos = (channel.sample_pos >> 32) as usize;
        if let Some((rs, rl)) = sample.repeat {
            while pos >= rs + rl {
                pos -= rl;
                channel.sample_pos -= (rl as u64) << 32;
            }
        } else if pos >= sample.data.len() {
            return 0;
        }
        channel.sample_pos += channel.sample_bytes_per_frame;
        let mut val = sample.data[pos] as i32;
        if val >= 0x80 {
            val -= 0x100;
        }
        val <<= 16;
        val *= channel.volume as i32;
        val
    }
}
