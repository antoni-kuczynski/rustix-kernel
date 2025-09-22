
/*
 * Vga buffer has typically two dimensional array
 * with size of 25 rows and 80 columns which is directly
 * rendered to the screen.
 * Each array entry discribes a single screen character with 
 * following format:
 *
 * Bit(s)   Value
 * 0-7      ASCII (code page 473 to be specific) code point
 * 8-11     Foreground color
 * 12-14    Background color
 * 15       Blink
 *
 * Colors that are available where Bit 4 is the bright bit:
 * (Note: For background color this bit is repurposed as the blink bit)
 *
 * Number   Color       Number          BrightColor
 *                      + Bright Bit
 *
 * 0x0      Black       0x8             Dark Gray
 * 0x1      Blue        0x9             Light Blue
 * 0x2      Green       0xa             Light Green
 * 0x3      Cyan        0xb             Light Cyan
 * 0x4      Red         0xc             Light Red
 * 0x5      Magenta     0xd             Pink
 * 0x6      Brown       0xe             Yellow
 * 0x7      LightGray   0xf             White
 */



