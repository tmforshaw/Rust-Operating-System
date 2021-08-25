#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Colour {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct ColourCode(u8);

impl ColourCode {
    fn new(foreground: Colour, background: Colour) -> Self {
        Self((background as u8) << 4 | (foreground as u8))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar {
    ascii_char: u8,
    colour_code: ColourCode,
}

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;

use volatile::Volatile;

#[repr(transparent)]
struct Buffer {
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

pub struct Writer {
    column_pos: usize,
    colour_code: ColourCode,
    buffer: &'static mut Buffer,
}

#[allow(dead_code)]
impl Writer {
    fn new() -> Self {
        Self {
            column_pos: 0,
            colour_code: ColourCode::new(Colour::Green, Colour::Black),
            buffer: unsafe { &mut *(0xB8000 as *mut Buffer) },
        }
    }

    fn new_from_colour(p_colour_code: ColourCode) -> Self {
        Self {
            column_pos: 0,
            colour_code: p_colour_code,
            buffer: unsafe { &mut *(0xB8000 as *mut Buffer) },
        }
    }

    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            // If it's a new line char
            b'\n' => self.new_line(),

            // Check if it is a printable char
            b' '..=b'~' => {
                // New line if overflown
                if self.column_pos >= BUFFER_WIDTH {
                    self.new_line();
                }

                // Change char in buffer
                self.buffer.chars[BUFFER_HEIGHT - 1][self.column_pos].write(ScreenChar {
                    ascii_char: byte,
                    colour_code: self.colour_code,
                });

                // Increment column position
                self.column_pos += 1;
            }

            // Not part of the printable chars
            _ => self.write_byte(0xFE), // Print a block char to show error
        }
    }

    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            self.write_byte(byte);
        }
    }

    fn new_line(&mut self) {
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                // Get chr from buffer
                let chr = self.buffer.chars[row][col].read();

                // Move chr up one row
                self.buffer.chars[row - 1][col].write(chr);
            }
        }

        self.clear_row(BUFFER_HEIGHT - 1);
        self.column_pos = 0;
    }

    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            ascii_char: b' ',
            colour_code: self.colour_code,
        };

        for col in 0..BUFFER_WIDTH {
            self.buffer.chars[row][col].write(blank);
        }
    }
}

impl core::fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

// Create a global instance of Writer using a lazily evaluated static

use lazy_static::lazy_static;
use spin::Mutex;

lazy_static! {
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer::new_from_colour(ColourCode::new(
        Colour::LightGreen,
        Colour::Black
    )));
}

// Create print macros for the VGA buffer

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga_buffer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    // Stop interrupts while the writer mutex is locked
    interrupts::without_interrupts(|| {
        WRITER.lock().write_fmt(args).unwrap();
    });
}

#[test_case]
fn test_println_output() {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    let s = "Some test string that fits on a single line";

    // Stop interrupts while the for the entire test
    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock();

        // Use writeln to write to a locked writer
        writeln!(writer, "\n{}", s).expect("writeln failed");
        for (i, c) in s.chars().enumerate() {
            let screen_char = writer.buffer.chars[BUFFER_HEIGHT - 2][i].read();
            assert_eq!(char::from(screen_char.ascii_char), c);
        }
    });
}
