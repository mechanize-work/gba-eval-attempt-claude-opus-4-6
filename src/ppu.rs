pub struct BgAffine {
    pub pa: i16,
    pub pb: i16,
    pub pc: i16,
    pub pd: i16,
    pub ref_x: u32,
    pub ref_y: u32,
    pub internal_x: i32,
    pub internal_y: i32,
}

impl BgAffine {
    pub fn new() -> Self {
        Self {
            pa: 0x100, pb: 0, pc: 0, pd: 0x100,
            ref_x: 0, ref_y: 0,
            internal_x: 0, internal_y: 0,
        }
    }
}

pub struct Ppu {
    pub dispcnt: u16,
    pub green_swap: u16,
    pub dispstat: u16,
    pub bgcnt: [u16; 4],
    pub bghofs: [u16; 4],
    pub bgvofs: [u16; 4],
    pub bg_affine: [BgAffine; 2],
    pub win0h: u16,
    pub win1h: u16,
    pub win0v: u16,
    pub win1v: u16,
    pub winin: u16,
    pub winout: u16,
    pub mosaic: u16,
    pub bldcnt: u16,
    pub bldalpha: u16,
    pub bldy: u16,
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            dispcnt: 0x0080,
            green_swap: 0,
            dispstat: 0,
            bgcnt: [0; 4],
            bghofs: [0; 4],
            bgvofs: [0; 4],
            bg_affine: [BgAffine::new(), BgAffine::new()],
            win0h: 0,
            win1h: 0,
            win0v: 0,
            win1v: 0,
            winin: 0,
            winout: 0,
            mosaic: 0,
            bldcnt: 0,
            bldalpha: 0,
            bldy: 0,
        }
    }

    pub fn on_vblank_end(&mut self) {
        self.bg_affine[0].internal_x = sign_extend_28(self.bg_affine[0].ref_x);
        self.bg_affine[0].internal_y = sign_extend_28(self.bg_affine[0].ref_y);
        self.bg_affine[1].internal_x = sign_extend_28(self.bg_affine[1].ref_x);
        self.bg_affine[1].internal_y = sign_extend_28(self.bg_affine[1].ref_y);
    }

    pub fn render_scanline(&mut self, line: u16, palette: &[u8], vram: &[u8], oam: &[u8], framebuffer: &mut [u32]) {
        let forced_blank = self.dispcnt & (1 << 7) != 0;
        if forced_blank {
            let offset = line as usize * 240;
            for x in 0..240 {
                framebuffer[offset + x] = 0xFF_FF_FF_FF;
            }
            return;
        }

        let mode = self.dispcnt & 7;
        let offset = line as usize * 240;

        let mut bg_lines: [[u16; 240]; 4] = [[0x8000; 240]; 4];
        let mut obj_line: [(u16, u8, bool); 240] = [(0x8000, 4, false); 240];

        match mode {
            0 => {
                for bg in 0..4 {
                    if self.dispcnt & (1 << (8 + bg)) != 0 {
                        self.render_text_bg(bg, line, vram, palette, &mut bg_lines[bg]);
                    }
                }
            }
            1 => {
                for bg in 0..2 {
                    if self.dispcnt & (1 << (8 + bg)) != 0 {
                        self.render_text_bg(bg, line, vram, palette, &mut bg_lines[bg]);
                    }
                }
                if self.dispcnt & (1 << 10) != 0 {
                    self.render_affine_bg(2, 0, line, vram, palette, &mut bg_lines[2]);
                }
            }
            2 => {
                if self.dispcnt & (1 << 10) != 0 {
                    self.render_affine_bg(2, 0, line, vram, palette, &mut bg_lines[2]);
                }
                if self.dispcnt & (1 << 11) != 0 {
                    self.render_affine_bg(3, 1, line, vram, palette, &mut bg_lines[3]);
                }
            }
            3 => {
                if self.dispcnt & (1 << 10) != 0 {
                    self.render_mode3(line, vram, &mut bg_lines[2]);
                }
            }
            4 => {
                if self.dispcnt & (1 << 10) != 0 {
                    self.render_mode4(line, vram, palette, &mut bg_lines[2]);
                }
            }
            5 => {
                if self.dispcnt & (1 << 10) != 0 {
                    self.render_mode5(line, vram, &mut bg_lines[2]);
                }
            }
            _ => {}
        }

        if self.dispcnt & (1 << 12) != 0 {
            self.render_sprites(line, vram, palette, oam, &mut obj_line);
        }

        self.compose_scanline(line, &bg_lines, &obj_line, palette, framebuffer);

        if mode >= 1 && self.dispcnt & (1 << 10) != 0 {
            self.bg_affine[0].internal_x += self.bg_affine[0].pb as i32;
            self.bg_affine[0].internal_y += self.bg_affine[0].pd as i32;
        }
        if mode == 2 && self.dispcnt & (1 << 11) != 0 {
            self.bg_affine[1].internal_x += self.bg_affine[1].pb as i32;
            self.bg_affine[1].internal_y += self.bg_affine[1].pd as i32;
        }
    }

    fn render_text_bg(&self, bg: usize, line: u16, vram: &[u8], _palette: &[u8], out: &mut [u16; 240]) {
        let cnt = self.bgcnt[bg];
        let char_base = ((cnt >> 2) & 3) as usize * 16384;
        let mosaic = cnt & (1 << 6) != 0;
        let color_256 = cnt & (1 << 7) != 0;
        let screen_base = ((cnt >> 8) & 0x1F) as usize * 2048;
        let size = (cnt >> 14) & 3;

        let (map_w, map_h) = match size {
            0 => (256u32, 256u32),
            1 => (512, 256),
            2 => (256, 512),
            3 => (512, 512),
            _ => (256, 256),
        };

        let scroll_x = self.bghofs[bg] as u32;
        let scroll_y = self.bgvofs[bg] as u32;

        let y = if mosaic {
            let mh = ((self.mosaic >> 4) & 0xF) as u32 + 1;
            ((line as u32 / mh) * mh).wrapping_add(scroll_y) & (map_h - 1)
        } else {
            (line as u32).wrapping_add(scroll_y) & (map_h - 1)
        };

        let tile_row = y / 8;
        let pixel_y = y & 7;

        for x in 0..240u32 {
            let px = if mosaic {
                let mw = (self.mosaic & 0xF) as u32 + 1;
                ((x / mw) * mw).wrapping_add(scroll_x) & (map_w - 1)
            } else {
                x.wrapping_add(scroll_x) & (map_w - 1)
            };

            let tile_col = px / 8;
            let pixel_x = px & 7;

            let screen_block = match size {
                0 => 0,
                1 => (tile_col / 32) as usize,
                2 => (tile_row / 32) as usize,
                3 => ((tile_row / 32) * 2 + tile_col / 32) as usize,
                _ => 0,
            };

            let local_col = tile_col & 31;
            let local_row = tile_row & 31;
            let map_idx = screen_base + screen_block * 2048 + (local_row * 32 + local_col) as usize * 2;

            if map_idx + 1 >= vram.len() { continue; }
            let entry = u16::from_le_bytes([vram[map_idx], vram[map_idx + 1]]);

            let tile_num = (entry & 0x3FF) as usize;
            let h_flip = entry & (1 << 10) != 0;
            let v_flip = entry & (1 << 11) != 0;
            let pal = ((entry >> 12) & 0xF) as u16;

            let py = if v_flip { 7 - pixel_y } else { pixel_y };
            let px_local = if h_flip { 7 - pixel_x } else { pixel_x };

            if color_256 {
                let tile_addr = char_base + tile_num * 64 + py as usize * 8 + px_local as usize;
                if tile_addr < vram.len() {
                    let color_idx = vram[tile_addr];
                    if color_idx != 0 {
                        out[x as usize] = color_idx as u16;
                    }
                }
            } else {
                let tile_addr = char_base + tile_num * 32 + py as usize * 4 + (px_local / 2) as usize;
                if tile_addr < vram.len() {
                    let byte = vram[tile_addr];
                    let color_idx = if px_local & 1 != 0 { byte >> 4 } else { byte & 0xF };
                    if color_idx != 0 {
                        out[x as usize] = (pal * 16 + color_idx as u16) as u16;
                    }
                }
            }
        }
    }

    fn render_affine_bg(&self, bg: usize, affine_idx: usize, _line: u16, vram: &[u8], _palette: &[u8], out: &mut [u16; 240]) {
        let cnt = self.bgcnt[bg];
        let char_base = ((cnt >> 2) & 3) as usize * 16384;
        let screen_base = ((cnt >> 8) & 0x1F) as usize * 2048;
        let wraparound = cnt & (1 << 13) != 0;
        let size = (cnt >> 14) & 3;

        let map_size = match size {
            0 => 128,
            1 => 256,
            2 => 512,
            3 => 1024,
            _ => 128,
        };
        let tiles = map_size / 8;

        let aff = &self.bg_affine[affine_idx];
        let mut ref_x = aff.internal_x;
        let mut ref_y = aff.internal_y;

        let pa = aff.pa as i32;
        let pc = aff.pc as i32;

        for x in 0..240 {
            let sx = ref_x >> 8;
            let sy = ref_y >> 8;

            ref_x += pa;
            ref_y += pc;

            let (tx, ty) = if wraparound {
                (((sx % map_size as i32) + map_size as i32) % map_size as i32,
                 ((sy % map_size as i32) + map_size as i32) % map_size as i32)
            } else {
                if sx < 0 || sy < 0 || sx >= map_size as i32 || sy >= map_size as i32 {
                    continue;
                }
                (sx, sy)
            };

            let tile_x = tx as u32 / 8;
            let tile_y = ty as u32 / 8;
            let pixel_x = tx as u32 & 7;
            let pixel_y = ty as u32 & 7;

            let map_idx = screen_base + (tile_y * tiles as u32 + tile_x) as usize;
            if map_idx >= vram.len() { continue; }
            let tile_num = vram[map_idx] as usize;

            let tile_addr = char_base + tile_num * 64 + pixel_y as usize * 8 + pixel_x as usize;
            if tile_addr >= vram.len() { continue; }
            let color_idx = vram[tile_addr];
            if color_idx != 0 {
                out[x] = color_idx as u16;
            }
        }
    }

    fn render_mode3(&self, line: u16, vram: &[u8], out: &mut [u16; 240]) {
        let y = line as usize;
        for x in 0..240 {
            let addr = (y * 240 + x) * 2;
            if addr + 1 < vram.len() {
                let color = u16::from_le_bytes([vram[addr], vram[addr + 1]]);
                out[x] = color | 0x8000;
            }
        }
    }

    fn render_mode4(&self, line: u16, vram: &[u8], _palette: &[u8], out: &mut [u16; 240]) {
        let frame = if self.dispcnt & (1 << 4) != 0 { 0xA000 } else { 0 };
        let y = line as usize;
        for x in 0..240 {
            let addr = frame + y * 240 + x;
            if addr < vram.len() {
                let color_idx = vram[addr];
                if color_idx != 0 {
                    out[x] = color_idx as u16;
                }
            }
        }
    }

    fn render_mode5(&self, line: u16, vram: &[u8], out: &mut [u16; 240]) {
        let frame = if self.dispcnt & (1 << 4) != 0 { 0xA000 } else { 0 };
        let y = line as usize;
        if y >= 128 { return; }
        for x in 0..160.min(240) {
            let addr = frame + (y * 160 + x) * 2;
            if addr + 1 < vram.len() {
                let color = u16::from_le_bytes([vram[addr], vram[addr + 1]]);
                out[x] = color | 0x8000;
            }
        }
    }

    fn render_sprites(&self, line: u16, vram: &[u8], _palette: &[u8], oam: &[u8], out: &mut [(u16, u8, bool); 240]) {
        let mapping_1d = self.dispcnt & (1 << 6) != 0;

        for i in (0..128).rev() {
            let attr0 = u16::from_le_bytes([oam[i * 8], oam[i * 8 + 1]]);
            let attr1 = u16::from_le_bytes([oam[i * 8 + 2], oam[i * 8 + 3]]);
            let attr2 = u16::from_le_bytes([oam[i * 8 + 4], oam[i * 8 + 5]]);

            let rot_scale = attr0 & (1 << 8) != 0;
            let double_or_disable = attr0 & (1 << 9) != 0;

            if !rot_scale && double_or_disable {
                continue;
            }

            let obj_mode = (attr0 >> 10) & 3;
            if obj_mode == 3 { continue; }
            let semi_transparent = obj_mode == 1;

            let color_256 = attr0 & (1 << 13) != 0;
            let shape = (attr0 >> 14) & 3;
            let size_bits = (attr1 >> 14) & 3;

            let (obj_w, obj_h) = sprite_size(shape, size_bits);

            let y = (attr0 & 0xFF) as i32;
            let y = if y >= 160 { y - 256 } else { y };

            let bounds_h = if rot_scale && double_or_disable { obj_h * 2 } else { obj_h };
            let bounds_w = if rot_scale && double_or_disable { obj_w * 2 } else { obj_w };

            let local_y = line as i32 - y;
            if local_y < 0 || local_y >= bounds_h as i32 { continue; }

            let x = (attr1 & 0x1FF) as i32;
            let x = if x >= 240 { x - 512 } else { x };

            let tile_num = (attr2 & 0x3FF) as usize;
            let priority = ((attr2 >> 10) & 3) as u8;
            let pal = ((attr2 >> 12) & 0xF) as u16;

            if rot_scale {
                let param_idx = ((attr1 >> 9) & 0x1F) as usize;
                let pa = i16::from_le_bytes([oam[param_idx * 32 + 6], oam[param_idx * 32 + 7]]) as i32;
                let pb = i16::from_le_bytes([oam[param_idx * 32 + 14], oam[param_idx * 32 + 15]]) as i32;
                let pc = i16::from_le_bytes([oam[param_idx * 32 + 22], oam[param_idx * 32 + 23]]) as i32;
                let pd = i16::from_le_bytes([oam[param_idx * 32 + 30], oam[param_idx * 32 + 31]]) as i32;

                let cx = obj_w as i32 / 2;
                let cy = obj_h as i32 / 2;
                let bcx = bounds_w as i32 / 2;
                let bcy = bounds_h as i32 / 2;

                let iy = local_y - bcy;

                for screen_x in 0..bounds_w as i32 {
                    let sx = x + screen_x;
                    if sx < 0 || sx >= 240 { continue; }

                    let ix = screen_x - bcx;

                    let tex_x = ((pa * ix + pb * iy) >> 8) + cx;
                    let tex_y = ((pc * ix + pd * iy) >> 8) + cy;

                    if tex_x < 0 || tex_x >= obj_w as i32 || tex_y < 0 || tex_y >= obj_h as i32 {
                        continue;
                    }

                    let color_idx = self.get_sprite_pixel(tile_num, tex_x as u32, tex_y as u32, obj_w, color_256, mapping_1d, vram);
                    if color_idx != 0 {
                        let final_color = if color_256 {
                            256 + color_idx as u16
                        } else {
                            256 + pal * 16 + color_idx as u16
                        };
                        let sx_u = sx as usize;
                        if out[sx_u].1 >= priority {
                            out[sx_u] = (final_color, priority, semi_transparent);
                        }
                    }
                }
            } else {
                let h_flip = attr1 & (1 << 12) != 0;
                let v_flip = attr1 & (1 << 13) != 0;

                let tex_y = if v_flip {
                    obj_h - 1 - local_y as u32
                } else {
                    local_y as u32
                };

                for screen_x in 0..obj_w as i32 {
                    let sx = x + screen_x;
                    if sx < 0 || sx >= 240 { continue; }

                    let tex_x = if h_flip {
                        obj_w - 1 - screen_x as u32
                    } else {
                        screen_x as u32
                    };

                    let color_idx = self.get_sprite_pixel(tile_num, tex_x, tex_y, obj_w, color_256, mapping_1d, vram);
                    if color_idx != 0 {
                        let final_color = if color_256 {
                            256 + color_idx as u16
                        } else {
                            256 + pal * 16 + color_idx as u16
                        };
                        let sx_u = sx as usize;
                        if out[sx_u].1 >= priority {
                            out[sx_u] = (final_color, priority, semi_transparent);
                        }
                    }
                }
            }
        }
    }

    fn get_sprite_pixel(&self, tile_num: usize, tex_x: u32, tex_y: u32, obj_w: u32, color_256: bool, mapping_1d: bool, vram: &[u8]) -> u8 {
        let tile_x = tex_x / 8;
        let tile_y = tex_y / 8;
        let pixel_x = tex_x & 7;
        let pixel_y = tex_y & 7;

        let tile_idx = if mapping_1d {
            if color_256 {
                tile_num + (tile_y * (obj_w / 8) + tile_x) as usize * 2
            } else {
                tile_num + (tile_y * (obj_w / 8) + tile_x) as usize
            }
        } else {
            if color_256 {
                (tile_num + tile_y as usize * 32 + tile_x as usize * 2) & 0x3FF
            } else {
                (tile_num + tile_y as usize * 32 + tile_x as usize) & 0x3FF
            }
        };

        let base = 0x10000;
        if color_256 {
            let addr = base + tile_idx * 32 + pixel_y as usize * 8 + pixel_x as usize;
            if addr < vram.len() { vram[addr] } else { 0 }
        } else {
            let addr = base + tile_idx * 32 + pixel_y as usize * 4 + (pixel_x / 2) as usize;
            if addr < vram.len() {
                let byte = vram[addr];
                if pixel_x & 1 != 0 { byte >> 4 } else { byte & 0xF }
            } else {
                0
            }
        }
    }

    fn compose_scanline(&self, line: u16, bg_lines: &[[u16; 240]; 4], obj_line: &[(u16, u8, bool); 240], palette: &[u8], framebuffer: &mut [u32]) {
        let offset = line as usize * 240;
        let mode = self.dispcnt & 7;

        let use_windows = self.dispcnt & ((1 << 13) | (1 << 14) | (1 << 15)) != 0;

        let bg_color = palette_color(palette, 0);

        let blend_mode = (self.bldcnt >> 6) & 3;
        let eva = ((self.bldalpha & 0x1F) as u32).min(16);
        let evb = (((self.bldalpha >> 8) & 0x1F) as u32).min(16);
        let evy = ((self.bldy & 0x1F) as u32).min(16);

        for x in 0..240usize {
            let win_flags = if use_windows {
                self.get_window_flags(x as u16, line)
            } else {
                0x3F
            };

            let mut top_color = bg_color;
            let mut top_priority = 4u8;
            let mut top_layer = 5u8;
            let mut second_color = bg_color;
            let mut second_layer = 5u8;

            let bg_order: [(usize, u8); 4] = [
                (0, (self.bgcnt[0] & 3) as u8),
                (1, (self.bgcnt[1] & 3) as u8),
                (2, (self.bgcnt[2] & 3) as u8),
                (3, (self.bgcnt[3] & 3) as u8),
            ];

            for &(bg_idx, prio) in bg_order.iter() {
                if self.dispcnt & (1 << (8 + bg_idx)) == 0 { continue; }
                if win_flags & (1 << bg_idx) == 0 { continue; }
                if bg_lines[bg_idx][x] == 0x8000 { continue; }

                let is_direct = mode >= 3 && bg_idx == 2 && bg_lines[bg_idx][x] & 0x8000 != 0;
                let color = if is_direct {
                    color_from_rgb5(bg_lines[bg_idx][x] & 0x7FFF)
                } else {
                    palette_color(palette, bg_lines[bg_idx][x] as usize)
                };

                if prio < top_priority || (prio == top_priority && bg_idx < top_layer as usize) {
                    second_color = top_color;
                    second_layer = top_layer;
                    top_color = color;
                    top_priority = prio;
                    top_layer = bg_idx as u8;
                } else if prio < 4 {
                    second_color = color;
                    second_layer = bg_idx as u8;
                }
            }

            let (obj_color_idx, obj_prio, obj_semi) = obj_line[x];
            if obj_color_idx != 0x8000 && (win_flags & (1 << 4)) != 0 {
                let obj_color = palette_color(palette, obj_color_idx as usize);
                if obj_prio <= top_priority {
                    second_color = top_color;
                    second_layer = top_layer;
                    top_color = obj_color;
                    top_priority = obj_prio;
                    top_layer = 4;
                } else {
                    second_color = obj_color;
                    second_layer = 4;
                }
            }

            let mut final_color = top_color;

            if win_flags & (1 << 5) != 0 {
                let first_target = is_blend_target(self.bldcnt, top_layer, false);
                let second_target = is_blend_target(self.bldcnt, second_layer, true);

                match blend_mode {
                    1 => {
                        if top_layer == 4 && obj_semi && second_target {
                            final_color = alpha_blend(top_color, second_color, eva, evb);
                        } else if first_target && second_target {
                            final_color = alpha_blend(top_color, second_color, eva, evb);
                        }
                    }
                    2 => {
                        if first_target {
                            final_color = brightness_increase(top_color, evy);
                        }
                    }
                    3 => {
                        if first_target {
                            final_color = brightness_decrease(top_color, evy);
                        }
                    }
                    _ => {}
                }

                if blend_mode != 1 && top_layer == 4 && obj_semi && second_target {
                    final_color = alpha_blend(top_color, second_color, eva, evb);
                }
            }

            framebuffer[offset + x] = final_color;
        }
    }

    fn get_window_flags(&self, x: u16, y: u16) -> u16 {
        if self.dispcnt & (1 << 13) != 0 {
            let x1 = (self.win0h >> 8) as u16;
            let x2 = (self.win0h & 0xFF) as u16;
            let y1 = (self.win0v >> 8) as u16;
            let y2 = (self.win0v & 0xFF) as u16;

            let in_x = if x1 <= x2 { x >= x1 && x < x2 } else { x >= x1 || x < x2 };
            let in_y = if y1 <= y2 { y >= y1 && y < y2 } else { y >= y1 || y < y2 };

            if in_x && in_y {
                return self.winin & 0x3F;
            }
        }

        if self.dispcnt & (1 << 14) != 0 {
            let x1 = (self.win1h >> 8) as u16;
            let x2 = (self.win1h & 0xFF) as u16;
            let y1 = (self.win1v >> 8) as u16;
            let y2 = (self.win1v & 0xFF) as u16;

            let in_x = if x1 <= x2 { x >= x1 && x < x2 } else { x >= x1 || x < x2 };
            let in_y = if y1 <= y2 { y >= y1 && y < y2 } else { y >= y1 || y < y2 };

            if in_x && in_y {
                return (self.winin >> 8) & 0x3F;
            }
        }

        self.winout & 0x3F
    }
}

fn sprite_size(shape: u16, size: u16) -> (u32, u32) {
    match (shape, size) {
        (0, 0) => (8, 8),
        (0, 1) => (16, 16),
        (0, 2) => (32, 32),
        (0, 3) => (64, 64),
        (1, 0) => (16, 8),
        (1, 1) => (32, 8),
        (1, 2) => (32, 16),
        (1, 3) => (64, 32),
        (2, 0) => (8, 16),
        (2, 1) => (8, 32),
        (2, 2) => (16, 32),
        (2, 3) => (32, 64),
        _ => (8, 8),
    }
}

fn palette_color(palette: &[u8], idx: usize) -> u32 {
    let addr = idx * 2;
    if addr + 1 >= palette.len() { return 0xFF000000; }
    let c = u16::from_le_bytes([palette[addr], palette[addr + 1]]);
    color_from_rgb5(c)
}

fn color_from_rgb5(c: u16) -> u32 {
    let r5 = (c & 0x1F) as u32;
    let g5 = ((c >> 5) & 0x1F) as u32;
    let b5 = ((c >> 10) & 0x1F) as u32;
    let r = (r5 << 3) | (r5 >> 2);
    let g = (g5 << 3) | (g5 >> 2);
    let b = (b5 << 3) | (b5 >> 2);
    0xFF000000 | (b << 16) | (g << 8) | r
}

fn is_blend_target(bldcnt: u16, layer: u8, second: bool) -> bool {
    if layer > 5 { return false; }
    let bits = if second { (bldcnt >> 8) as u8 } else { bldcnt as u8 };
    bits & (1 << layer) != 0
}

fn alpha_blend(top: u32, bot: u32, eva: u32, evb: u32) -> u32 {
    let r1 = (top & 0xFF) >> 3;
    let g1 = ((top >> 8) & 0xFF) >> 3;
    let b1 = ((top >> 16) & 0xFF) >> 3;
    let r2 = (bot & 0xFF) >> 3;
    let g2 = ((bot >> 8) & 0xFF) >> 3;
    let b2 = ((bot >> 16) & 0xFF) >> 3;
    let r = ((r1 * eva + r2 * evb) / 16).min(31);
    let g = ((g1 * eva + g2 * evb) / 16).min(31);
    let b = ((b1 * eva + b2 * evb) / 16).min(31);
    0xFF000000 | ((b << 3 | b >> 2) << 16) | ((g << 3 | g >> 2) << 8) | (r << 3 | r >> 2)
}

fn brightness_increase(color: u32, evy: u32) -> u32 {
    let r = (color & 0xFF) >> 3;
    let g = ((color >> 8) & 0xFF) >> 3;
    let b = ((color >> 16) & 0xFF) >> 3;
    let r = (r + (31 - r) * evy / 16).min(31);
    let g = (g + (31 - g) * evy / 16).min(31);
    let b = (b + (31 - b) * evy / 16).min(31);
    0xFF000000 | ((b << 3 | b >> 2) << 16) | ((g << 3 | g >> 2) << 8) | (r << 3 | r >> 2)
}

fn brightness_decrease(color: u32, evy: u32) -> u32 {
    let r = (color & 0xFF) >> 3;
    let g = ((color >> 8) & 0xFF) >> 3;
    let b = ((color >> 16) & 0xFF) >> 3;
    let r = r - r * evy / 16;
    let g = g - g * evy / 16;
    let b = b - b * evy / 16;
    0xFF000000 | ((b << 3 | b >> 2) << 16) | ((g << 3 | g >> 2) << 8) | (r << 3 | r >> 2)
}

fn sign_extend_28(val: u32) -> i32 {
    let val = val & 0x0FFF_FFFF;
    if val & 0x0800_0000 != 0 {
        (val | 0xF000_0000) as i32
    } else {
        val as i32
    }
}
