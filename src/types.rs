use std::collections::HashMap;

pub struct State {
    pub windows: HashMap<u32, Window>,
    pub window_open: bool,
}

pub struct Window {
    pub id: u32,
    pub title: String,
    pub jpeg: Vec<u8>,
    pub jpeg_metrohash: u64,
    //pub jpeg_small: Vec<u8>,
    pub z: usize,
}
