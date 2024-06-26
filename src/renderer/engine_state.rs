use std::{cell::RefCell, rc::Rc};
use paste::paste;
use crate::GraphicsLevel;

use super::{DemoHistoryPlayback, DemoStateHistory};

macro_rules! impl_button {
   ($destination: ident, $key: ident, $idx: expr) => {
      pub fn $key(&self) -> f32 { self.$destination[$idx as usize] }
      paste!{pub fn [<down_ $key>](&mut self) { self.$destination[$idx as usize] = 1.0; } }
      paste!{pub fn [<up_ $key>](&mut self) {
         self.[<set_ $destination>]($idx as usize, -1.0);
      }}
      paste!{pub fn [<set_ $key>](&mut self, value: f32) {
         self.[<set_ $destination>]($idx as usize, value);
      }}
      paste!{pub fn [<raw_idx_ $key>](&mut self) -> usize { $idx as usize } }
   };
}

macro_rules! impl_buttons {
   ($fn_name: ident, $destination: ident) => {
      paste!{pub fn $fn_name(&mut self, raw_idx: usize, value: f32) {
         self.$destination[raw_idx] = value;
         if value < 0.0 {
            self.[<$destination _to_dismiss>][self.[<$destination _to_dismiss_idx>]] = raw_idx;
            self.[<$destination _to_dismiss_idx>] += 1;
         }
      }}
   }
}

#[derive(Default, Clone, Copy)]
pub struct MouseState {
   button: [f32; 8],
   button_to_dismiss: [usize; 10],
   button_to_dismiss_idx: usize,
   pub wheel: (f32, f32), // (horizontal, vertical), -1 == left/down, 1 == right/up
   pub canvas_position_px: (i32, i32), // origin at top-left
}

impl MouseState {
   impl_buttons!(set_button, button);
   impl_button!(button, left, 0);
   impl_button!(button, middle, 0);
   impl_button!(button, right, 0);
}

#[derive(Default, Clone, Copy)]
pub struct KeyboardState {
   pub shift: bool,
   pub alt: bool,
   pub ctrl: bool,
   letter: [f32; 32],
   letter_to_dismiss: [usize; 32],
   letter_to_dismiss_idx: usize,
   digit: [f32; 10], // 0 1 2 3 4 5 6 7 8 9
   digit_to_dismiss: [usize; 10],
   digit_to_dismiss_idx: usize,
   punctuation: [f32; 32],
   punctuation_to_dismiss: [usize; 32],
   punctuation_to_dismiss_idx: usize,
   fkey: [f32; 12],
   fkey_to_dismiss: [usize; 12],
   fkey_to_dismiss_idx: usize,
}

impl KeyboardState {
   impl_buttons!(set_letter, letter);
   impl_buttons!(set_digit, digit);
   impl_buttons!(set_fkey, fkey);
   impl_buttons!(set_punctuation, punctuation);

   impl_button!(letter, a, 00);
   impl_button!(letter, b, 01);
   impl_button!(letter, c, 02);
   impl_button!(letter, d, 03);
   impl_button!(letter, e, 04);
   impl_button!(letter, f, 05);
   impl_button!(letter, g, 06);
   impl_button!(letter, h, 07);
   impl_button!(letter, i, 08);
   impl_button!(letter, j, 09);
   impl_button!(letter, k, 10);
   impl_button!(letter, l, 11);
   impl_button!(letter, m, 12);
   impl_button!(letter, n, 13);
   impl_button!(letter, o, 14);
   impl_button!(letter, p, 15);
   impl_button!(letter, q, 16);
   impl_button!(letter, r, 17);
   impl_button!(letter, s, 18);
   impl_button!(letter, t, 19);
   impl_button!(letter, u, 20);
   impl_button!(letter, v, 21);
   impl_button!(letter, w, 22);
   impl_button!(letter, x, 23);
   impl_button!(letter, y, 24);
   impl_button!(letter, z, 25);

   impl_button!(digit, d0, 0);
   impl_button!(digit, d1, 1);
   impl_button!(digit, d2, 2);
   impl_button!(digit, d3, 3);
   impl_button!(digit, d4, 4);
   impl_button!(digit, d5, 5);
   impl_button!(digit, d6, 6);
   impl_button!(digit, d7, 7);
   impl_button!(digit, d8, 8);
   impl_button!(digit, d9, 9);

   impl_button!(punctuation, backquote,         00);
   impl_button!(punctuation, comma,             01);
   impl_button!(punctuation, dot,               02);
   impl_button!(punctuation, bracket_left,      03);
   impl_button!(punctuation, bracket_right,     04);
}

#[derive(Clone, Default)]
struct DerivedDynamicState {
   pub time_since_startup_sec: f64,
   pub time_now_sec:   f64,
   pub time_prev_sec:  f64,
   pub time_delta_sec: f64,
   pub frame_rate: f32,
   pub mouse_viewport_position_px: (i32, i32), // origin at bottom-left
}

#[derive(Clone, Default)]
struct DerivedStableState {
   pub aspect_ratio: f32,
}

#[derive(Clone)]
struct StableState {
   pub screen_size: (u32, u32),
   pub graphics_level: GraphicsLevel,
   pub debug_mode: Option<u16>,
}

pub struct ExternalState {
   // dynamic
   mouse: Rc<RefCell<MouseState>>,
   keyboard: Rc<RefCell<KeyboardState>>,
   time_delta_limit_ms: f64,
   absolute_time_startup_ms: f64,
   absolute_time_tick_ms: f64,
   time_now_ms:    f64,
   time_prev_ms:   f64,
   time_delta_ms:  f64,
   frame_idx: usize,
   derived: DerivedDynamicState,

   // stable
   stable: StableState,
   is_stable_updated: bool,
   derived_stable: DerivedStableState,
}

#[derive(Default, Clone, Copy)]
pub struct ExternalStateData {
   // dynamic
   pub mouse: MouseState,
   pub keyboard: KeyboardState,
   pub time_delta_limit_ms: f64,
   pub absolute_time_startup_ms: f64,
   pub absolute_time_tick_ms:   f64,
   pub time_now_ms:    f64,
   pub time_prev_ms:   f64,
   pub time_delta_ms:  f64,
   pub frame_idx: usize,

   // stable
   pub screen_size: (u32, u32),
   pub graphics_level: GraphicsLevel,
   pub debug_mode: Option<u16>,
}

#[allow(unused)]
impl ExternalState {

   pub fn new(absolute_time_startup_ms: f64) -> Self {
      let mut state = ExternalState::default();
      state.absolute_time_startup_ms = absolute_time_startup_ms;
      state
   }

   pub fn data(&self) -> ExternalStateData {
      ExternalStateData {
         mouse: self.mouse.borrow().clone(),
         keyboard: self.keyboard.borrow().clone(),
         time_delta_limit_ms: self.time_delta_limit_ms.clone(),
         absolute_time_startup_ms: self.absolute_time_startup_ms.clone(),
         absolute_time_tick_ms: self.absolute_time_tick_ms.clone(),
         time_now_ms: self.time_now_ms.clone(),
         time_prev_ms: self.time_prev_ms.clone(),
         time_delta_ms: self.time_delta_ms.clone(),
         frame_idx: self.frame_idx.clone(),
         screen_size: self.stable.screen_size.clone(),
         graphics_level: self.stable.graphics_level.clone(),
         debug_mode: self.stable.debug_mode.clone(),
      }
   }

   pub fn reset(&mut self) {
      self.time_now_ms = Default::default();
      self.time_prev_ms = Default::default();
      self.time_delta_ms = Default::default();
      self.frame_idx = Default::default();
      self.update_derived_state();
   }

   pub fn mouse_unit_position(&self) -> (f32, f32) {
      let px_pos = self.mouse_viewport_position_px();
      return (
         px_pos.0 as f32 / self.stable.screen_size.0 as f32,
         px_pos.1 as f32 / self.stable.screen_size.1 as f32,
      )
   }

   pub fn update_derived_state(&mut self) {
      let now = self.time_now_ms * 0.001;
      let then = self.time_prev_ms * 0.001;
      let delta = now - then;
      
      let mouse_canvas_position_px = self.mouse.borrow().canvas_position_px;
      self.derived = DerivedDynamicState {
         time_since_startup_sec: (self.absolute_time_tick_ms - self.absolute_time_startup_ms)*0.001,
         time_now_sec: now,
         time_prev_sec: then,
         time_delta_sec: delta,
         frame_rate: (1.0 / delta) as f32,
         mouse_viewport_position_px: (
            mouse_canvas_position_px.0,
            self.stable.screen_size.1 as i32 - mouse_canvas_position_px.1
         ),
      };
      if self.is_stable_updated {
         self.derived_stable = DerivedStableState {
            aspect_ratio: self.stable.screen_size.0 as f32 / self.stable.screen_size.1 as f32,
         }
      }
   }

   #[inline] pub fn screen_size(&self) -> (u32, u32) { self.stable.screen_size }
   #[inline] pub fn graphics_level(&self) -> GraphicsLevel { self.stable.graphics_level }
   #[inline] pub fn debug_mode(&self) -> Option<u16> { self.stable.debug_mode }
   #[inline] pub fn aspect_ratio(&self) -> f32 { self.derived_stable.aspect_ratio }
   #[inline] pub fn is_stable_updated(&self) -> bool { self.is_stable_updated }

   #[inline] pub fn mouse(&self) -> &Rc<RefCell<MouseState>> { &self.mouse }
   #[inline] pub fn keyboard(&self) -> &Rc<RefCell<KeyboardState>> { &self.keyboard }
   #[inline] pub fn absolute_time_startup_ms(&self) -> f64 { self.absolute_time_startup_ms }
   #[inline] pub fn absolute_time_tick_ms(&self) -> f64 { self.absolute_time_tick_ms }
   // pub fn time_of_tick_ms(&self) -> f64 { self.time_of_tick_ms }
   #[inline] pub fn time_now_ms(&self) -> f64 { self.time_now_ms }
   #[inline] pub fn time_prev_ms(&self) -> f64 { self.time_prev_ms }
   #[inline] pub fn time_delta_ms(&self) -> f64 { self.time_delta_ms }
   #[inline] pub fn time_delta_limit_ms(&self) -> f64 { self.time_delta_limit_ms }
   #[inline] pub fn frame_idx(&self) -> usize { self.frame_idx }
   
   #[inline] pub fn time_since_startup_ms(&self) -> f64 { self.derived.time_since_startup_sec }
   #[inline] pub fn time_now_sec(&self) -> f64 { self.derived.time_now_sec }
   #[inline] pub fn time_prev_sec(&self) -> f64 { self.derived.time_prev_sec }
   #[inline] pub fn time_delta_sec(&self) -> f64 { self.derived.time_delta_sec }
   #[inline] pub fn frame_rate(&self) -> f32 { self.derived.frame_rate }
   #[inline] pub fn mouse_viewport_position_px(&self) -> (i32, i32) { self.derived.mouse_viewport_position_px }

   pub fn set_time_delta_limit_ms(&mut self, time_delta_limit_ms: f64) {
      self.time_delta_limit_ms = time_delta_limit_ms;
   }

   pub fn set_screen_size(&mut self, (width_px, height_px): (u32, u32)) {
      self.stable.screen_size = (width_px, height_px);
      self.is_stable_updated = true;
   }

   pub fn set_graphics_level(&mut self, graphics_level: GraphicsLevel) {
      self.stable.graphics_level = graphics_level;
      self.is_stable_updated = true;
   }

   pub fn set_debug_mode(&mut self, debug_mode: Option<u16>) {
      self.stable.debug_mode = debug_mode;
      self.is_stable_updated = true;
   }

   pub fn override_time(&mut self, tick_timestamp_ms: f64, now_time_ms: f64, frame_idx: usize) {
      self.frame_idx = frame_idx;
      self.absolute_time_tick_ms = tick_timestamp_ms;
      self.time_delta_ms = 0.0; // .max(1)
      self.time_prev_ms  = now_time_ms;
      self.time_now_ms   = now_time_ms;
      self.update_derived_state();
   }

   pub fn tick(&mut self, tick_timestamp_ms: f64) {
      self.frame_idx += 1;
      self.time_delta_ms = tick_timestamp_ms - self.absolute_time_tick_ms;
      self.absolute_time_tick_ms = tick_timestamp_ms;
      self.time_prev_ms  = self.time_now_ms;
      self.time_now_ms   += self.time_delta_ms;
      self.update_derived_state();
      // if (self.time_delta_ms != 0.0) {
      //    log::info!("dt {} {}", self.time_delta_ms, self.time_now_ms);
      // }
   }

   pub fn tick_from_delta(&mut self, tick_delta_ms: f64) {
      self.tick(self.time_prev_ms + tick_delta_ms);
   }

   pub fn dismiss_events(&mut self) {
      self.is_stable_updated = false;

      let mut mouse = self.mouse.borrow_mut();
      for i in 0..mouse.button_to_dismiss_idx {
         let state_idx = mouse.button_to_dismiss[i];
         mouse.button[state_idx] = 0.0;
      }
      mouse.button_to_dismiss_idx = 0;

      let mut keyboard = self.keyboard.borrow_mut();

      for i in 0..keyboard.letter_to_dismiss_idx {
         let state_idx = keyboard.letter_to_dismiss[i];
         keyboard.letter[state_idx] = 0.0;
      }
      keyboard.letter_to_dismiss_idx = 0;

      for i in 0..keyboard.digit_to_dismiss_idx {
         let state_idx = keyboard.digit_to_dismiss[i];
         keyboard.digit[state_idx] = 0.0;
      }
      keyboard.digit_to_dismiss_idx = 0;

      for i in 0..keyboard.fkey_to_dismiss_idx {
         let state_idx = keyboard.fkey_to_dismiss[i];
         keyboard.fkey[state_idx] = 0.0;
      }
      keyboard.fkey_to_dismiss_idx = 0;

      for i in 0..keyboard.punctuation_to_dismiss_idx {
         let state_idx = keyboard.punctuation_to_dismiss[i];
         keyboard.punctuation[state_idx] = 0.0;
      }
      keyboard.punctuation_to_dismiss_idx = 0;
   }

   fn dismiss_input_event(input_axis: &mut f32) {
      if *input_axis < 0.0 { *input_axis = 0.0; }
   }
}

impl Default for ExternalState {
   fn default() -> Self {
      Self {
         mouse: Rc::new(RefCell::new(Default::default())),
         keyboard: Rc::new(RefCell::new(Default::default())),
         time_delta_limit_ms: Default::default(),
         absolute_time_startup_ms: Default::default(),
         absolute_time_tick_ms: Default::default(),
         time_delta_ms: Default::default(),
         time_now_ms: Default::default(),
         time_prev_ms: Default::default(),
         frame_idx: Default::default(),
         derived: Default::default(),
         is_stable_updated: true,
         stable: StableState {
            screen_size: (1, 1),
            graphics_level: Default::default(),
            debug_mode: Default::default(),
         } ,
         derived_stable: Default::default(),
      }
   }
}

#[allow(unused)]
pub struct FrameStateRef<'a> {
    pub demo_state_history: &'a mut DemoStateHistory,
    pub demo_history_playback: &'a mut DemoHistoryPlayback,
    pub demo_state: &'a mut ExternalState,
    pub previous_timestamp_ms: f64,
    pub now_timestamp_ms: f64,
}

pub fn handle_keyboard<'a>(keyboard: KeyboardState, state: FrameStateRef<'a>) {
   if state.demo_state.debug_mode().is_some() && keyboard.m() < 0.0 {
      if state.demo_history_playback.is_playing_back() {
         if let Some((_, resume_time_ms)) = state.demo_history_playback.cancel_playback() {
            let frame_idx = 0;
            state.demo_state_history.reset_history();
            state.demo_state.override_time(state.now_timestamp_ms, resume_time_ms, frame_idx);
         }
      } else {
         state.demo_history_playback.start_playback(
            state.now_timestamp_ms, state.demo_state.time_now_ms());
      }
   }
   if keyboard.comma() < 0.0 || keyboard.comma() > 0.0 && keyboard.shift {
       state.demo_history_playback.play_back(&state.demo_state_history);
   }
   if keyboard.dot() < 0.0 || keyboard.dot() > 0.0 && keyboard.shift {
       state.demo_history_playback.play_forward(&state.demo_state_history);
   }
   if keyboard.backquote() < 0.0 && keyboard.ctrl {
      state.demo_state.set_debug_mode(None);
   }
   for i in 0..keyboard.digit.len() {
      if keyboard.digit[i] < 0.0 && keyboard.ctrl {
         state.demo_state.set_debug_mode(Some(i as u16));
         println!("DEBUG MODE {}", i);
      }
   }
}