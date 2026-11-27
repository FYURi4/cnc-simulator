// crates/engine-core/src/gcode.rs
use glam::Vec3;

#[derive(Debug, Clone)]
pub struct MotionSegment {
    pub from: Vec3,
    pub to: Vec3,
    pub rapid: bool, // G0=true, G1=false
}

#[derive(Debug, Default)]
pub struct GCodeProgram {
    pub segments: Vec<MotionSegment>,
    pub errors: Vec<String>,
}

impl GCodeProgram {
    pub fn parse(source: &str) -> Self {
        let mut program = GCodeProgram::default();
        let mut pos = Vec3::ZERO;
        let mut absolute = true;
        let mut rapid = true;

        for (lineno, line) in source.lines().enumerate() {
            let clean = line
                .split(';')
                .next()
                .unwrap_or("")
                .split('(')
                .next()
                .unwrap_or("")
                .trim();
            if clean.is_empty() {
                continue;
            }

            let mut words: Vec<(char, f32)> = Vec::new();
            let mut chars = clean.chars().peekable();
            let mut bad = false;
            while let Some(c) = chars.next() {
                if c.is_whitespace() {
                    continue;
                }
                let letter = c.to_ascii_uppercase();
                let mut num = String::new();
                while let Some(&n) = chars.peek() {
                    if n.is_ascii_digit() || n == '-' || n == '+' || n == '.' {
                        num.push(n);
                        chars.next();
                    } else {
                        break;
                    }
                }
                match num.parse::<f32>() {
                    Ok(v) => words.push((letter, v)),
                    Err(_) => {
                        bad = true;
                        break;
                    }
                }
            }
            if bad {
                program
                    .errors
                    .push(format!("line {}: bad syntax", lineno + 1));
                continue;
            }

            let g = words
                .iter()
                .find(|(l, _)| *l == 'G')
                .map(|(_, v)| *v as i32);

            match g {
                Some(90) => absolute = true,
                Some(91) => absolute = false,
                Some(0) => rapid = true,
                Some(1) => rapid = false,
                Some(2) | Some(3) => {
                    program
                        .errors
                        .push(format!("line {}: arcs unsupported", lineno + 1));
                }
                _ => {}
            }

            let has_motion = matches!(g, Some(0) | Some(1))
                || words.iter().any(|(l, _)| matches!(l, 'X' | 'Y' | 'Z'));

            if has_motion {
                let mut target = pos;
                for (l, v) in &words {
                    match l {
                        'X' => target.x = if absolute { *v } else { pos.x + v },
                        'Y' => target.y = if absolute { *v } else { pos.y + v },
                        'Z' => target.z = if absolute { *v } else { pos.z + v },
                        _ => {}
                    }
                }
                if target != pos {
                    program.segments.push(MotionSegment {
                        from: pos,
                        to: target,
                        rapid,
                    });
                    pos = target;
                }
            }
        }
        program
    }
}
