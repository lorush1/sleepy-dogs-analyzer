use std::collections::HashMap;

pub struct Canvas {
    width: usize,
    height: usize,
    grid: Vec<char>,
}

impl Canvas {
    pub fn new(width: usize, height: usize) -> Self {
        let size = width.saturating_mul(height);
        let grid = vec![' '; size];
        Self {
            width,
            height,
            grid,
        }
    }

    pub fn clear(&mut self) {
        for cell in self.grid.iter_mut() {
            *cell = ' ';
        }
    }

    fn index(&self, x: usize, y: usize) -> Option<usize> {
        if x >= self.width || y >= self.height {
            None
        } else {
            Some(y * self.width + x)
        }
    }

    pub fn set(&mut self, x: usize, y: usize, ch: char) {
        if let Some(idx) = self.index(x, y) {
            self.grid[idx] = ch;
        }
    }

    pub fn place_text(&mut self, mut x: usize, y: usize, text: &str) {
        for ch in text.chars() {
            if x >= self.width {
                break;
            }
            self.set(x, y, ch);
            x += 1;
        }
    }

    pub fn draw_line(&mut self, from: (usize, usize), to: (usize, usize), ch: char) {
        let (mut x0, mut y0) = (from.0 as isize, from.1 as isize);
        let (x1, y1) = (to.0 as isize, to.1 as isize);
        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx - dy;
        loop {
            if x0 >= 0 && y0 >= 0 {
                self.set(x0 as usize, y0 as usize, ch);
            }
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = err * 2;
            if e2 > -dy {
                err -= dy;
                x0 += sx;
            }
            if e2 < dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    pub fn draw_box(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        border: char,
    ) {
        if width == 0 || height == 0 {
            return;
        }
        let right = x.saturating_add(width - 1);
        let bottom = y.saturating_add(height - 1);
        for col in x..=right {
            self.set(col, y, border);
            self.set(col, bottom, border);
        }
        for row in y..=bottom {
            self.set(x, row, border);
            self.set(right, row, border);
        }
    }

    pub fn render(&self) -> String {
        if self.width == 0 {
            return String::new();
        }
        let mut output = String::new();
        for row in self.grid.chunks(self.width) {
            for &ch in row {
                output.push(ch);
            }
            output.push('\n');
        }
        output
    }
}

pub struct MurderBoard {
    pub suspects: Vec<SuspectNode>,
    pub evidence: Vec<EvidenceMarker>,
    pub connections: Vec<Connection>,
}

pub struct SuspectNode {
    pub name: String,
    pub guilt_score: f32,
    pub position: (u16, u16),
    pub is_selected: bool,
}

pub struct EvidenceMarker {
    pub label: String,
    pub position: (u16, u16),
}

pub struct Connection {
    pub from_suspect: String,
    pub to_suspect: String,
    pub confidence: f32,
    pub label: String,
}

impl MurderBoard {
    pub fn draw_murder_board(&self, canvas: &mut Canvas) -> String {
        canvas.clear();
        let mut centers = HashMap::new();
        const BOX_WIDTH: usize = 15;
        const BOX_HEIGHT: usize = 5;
        for suspect in &self.suspects {
            let x = suspect.position.0 as usize;
            let y = suspect.position.1 as usize;
            let border = if suspect.is_selected { 'O' } else { '#' };
            canvas.draw_box(x, y, BOX_WIDTH, BOX_HEIGHT, border);
            let guilt_text = format!("{:.1}%", suspect.guilt_score * 100.0);
            let title_text = if suspect.name.len() > 12 {
                suspect.name.chars().take(12).collect::<String>()
            } else {
                suspect.name.clone()
            };
            canvas.place_text(x + 2, y + 1, &title_text);
            canvas.place_text(x + 2, y + 2, &guilt_text);
            let center = (
                x.saturating_add(BOX_WIDTH / 2),
                y.saturating_add(BOX_HEIGHT / 2),
            );
            centers.insert(suspect.name.clone(), center);
        }
        for marker in &self.evidence {
            let x = marker.position.0 as usize;
            let y = marker.position.1 as usize;
            canvas.set(x, y, '*');
            let label = if marker.label.len() > 10 {
                marker.label.chars().take(10).collect::<String>()
            } else {
                marker.label.clone()
            };
            canvas.place_text(x + 1, y, &label);
        }
        for connection in &self.connections {
            if let (Some(&from), Some(&to)) = (
                centers.get(&connection.from_suspect),
                centers.get(&connection.to_suspect),
            ) {
                let char_token = connection_char(&connection.label, connection.confidence);
                canvas.draw_line(from, to, char_token);
                let compound = format!("~{}~", connection.label);
                let safe_label = if compound.len() > 16 {
                    compound.chars().take(16).collect::<String>()
                } else {
                    compound
                };
                let midx = from.0.saturating_add(to.0) / 2;
                let midy = from.1.saturating_add(to.1) / 2;
                let startx = midx.saturating_sub(safe_label.len() / 2);
                canvas.place_text(startx, midy, &safe_label);
            }
        }
        canvas.render()
    }
}

fn connection_char(label: &str, confidence: f32) -> char {
    let clue = label.to_lowercase();
    let base = if clue.contains("conflict") {
        'R'
    } else if clue.contains("alibi") {
        'B'
    } else {
        'G'
    };
    if confidence > 0.8 {
        base
    } else if confidence > 0.5 {
        match base {
            'R' => 'r',
            'B' => 'b',
            _ => 'g',
        }
    } else {
        '.'
    }
}
