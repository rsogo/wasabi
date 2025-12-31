#![no_std]
#![no_main]
#![feature(offset_of)]

use core::arch::asm;
use core::cmp::min;
use core::mem::offset_of;
use core::mem::size_of;
use core::panic::PanicInfo;
use core::ptr::null_mut;

type EfiVoid = u8;
type EfiHandle = u64;
type Result<T> = core::result::Result<T, &'static str>;

// no_mangleを指定することで、コンパイル時の名前の変更を防ぐ。
// UEFIのエントリポイント
// _image_handle: UEFIのイメージハンドル
// efi_system_table: UEFIのシステムテーブルへのポインタ
#[no_mangle]
fn efi_main(_image_handle: EfiHandle, efi_system_table: &EfiSystemTable) -> ! {
    
    let mut vram: VramBufferInfo = init_vram(efi_system_table).expect("init_vram failed");

    let vw = vram.width;
    let vh = vram.height;

    fill_rect(&mut vram, 0, 0, vw, vh, 0x00_00_00).expect("fill_rect failed");
    fill_rect(&mut vram, 32, 32, 32, 32, 0x00_00_ff).expect("fill_rect failed");
    fill_rect(&mut vram, 64, 64, 64, 64, 0x00_ff_00).expect("fill_rect failed");
    fill_rect(&mut vram, 128, 128, 128, 128, 0xff_00_00).expect("fill_rect failed");
    
    for i in 0..256 {
        let _ = draw_point(&mut vram, i, i, 0x01_01_01);
    }

    loop {
        // 待機
        hlt();
    }
}

unsafe fn unchecked_draw_point<T: Bitmap>(buf: &mut T, x: i64, y: i64, color: u32) {

    // X, Y座標から、ピクセルのアドレスを計算して色を書き込む
    *buf.unchecked_pixel_at_mut(x, y) = color;
}

fn draw_point<T: Bitmap>(
    buf: &mut T,
    x: i64,
    y: i64,
    color: u32
) -> Result<()> {
    *(buf.pixel_at_mut(x, y).ok_or("Out of Range")?) = color;
    Ok(())
}

fn fill_rect<T: Bitmap>(
    buf: &mut T,
    px: i64,
    py: i64,
    w: i64,
    h: i64,
    color: u32
) -> Result<()> {
    if !buf.is_in_x_range(px)
        || !buf.is_in_y_range(py)
        || !buf.is_in_x_range(px + w - 1)
        || !buf.is_in_y_range(py + h - 1)
    {
        return Err("Out of range");
    }

    for y in py..(py + h) {
        for x in px..(px + w) {
            unsafe {
                unchecked_draw_point(buf, x, y, color);
            }
        }
    }
    Ok(())
}

// #[repr(C)]はC言語のメモリレイアウトに合わせるためにつける
// 付けないとRustで最適化されて、どこにあるのか予測不可能になる
#[repr(C)]
struct EfiBootServiceTable {
    // Define the structure of the EFI Boot Services Table
    reserved0: [u64; 40],
    locate_protocol: extern "win64" fn(
        protocol: *const EfiGuid,
        registration: *const EfiVoid,
        interface: *mut *mut EfiVoid,
    ) -> EfiStatus,
}

// 構造体のフィールドのオフセットを確認
// こうすることで、コンパイル時にチェックできる
// 例えば、新しいフィールドを前に追加したときにオフセットが意図してズレたときに気づける
const _: () = assert!(offset_of!(EfiBootServiceTable, locate_protocol) == 320);

#[repr(C)]
struct EfiSystemTable {
    // Define the structure of the EFI System Table
    _reserved0: [u64; 12],
    pub boot_services: &'static EfiBootServiceTable,
}

const _: () = assert!(offset_of!(EfiSystemTable, boot_services) == 96);

const EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID: EfiGuid = EfiGuid {
    data0: 0x9042a9de,
    data1: 0x23dc,
    data2: 0x4a38,
    data3: [0x96, 0xfb, 0x7a, 0xde, 0xd0, 0x80, 0x51, 0x6a],
};

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct EfiGuid {
    pub data0: u32,
    pub data1: u16,
    pub data2: u16,
    pub data3: [u8; 8],
}

#[repr(C)]
#[derive(Debug)]
struct EfiGraphicsOutputProtocol<'a> {
    reserved: [u64; 3],
    pub mode: &'a EfiGraphicsOutputProtocolMode<'a>,
}

#[repr(C)]
#[derive(Debug)]
struct EfiGraphicsOutputProtocolMode<'a> {
    pub max_mode: u32,
    pub mode: u32,
    pub info: &'a EfiGraphicsOutputProtocolPixelInfo,
    pub size_of_info: u64,
    pub frame_buffer_base: usize,
    pub frame_buffer_size: usize,
}

#[repr(C)]
#[derive(Debug)]
struct EfiGraphicsOutputProtocolPixelInfo {
    version: u32,
    pub horizontal_resolution: u32,
    pub vertical_resolution: u32,
    pub _padding0: [u32; 5],
    pub pixels_per_scan_line: u32, // 水平方向に含まれる画素数
}

const _: () = assert!(size_of::<EfiGraphicsOutputProtocolPixelInfo>() == 36);

fn locate_graphic_protolocol<'a>(
    efi_system_table: &EfiSystemTable,
) -> Result<&'a EfiGraphicsOutputProtocol<'a>> {

    // EfiGraphicsOutputProtocolへのポインタを格納するための変数
    let mut graphic_output_protocol = null_mut::<EfiGraphicsOutputProtocol>();

    // EFI_GRAPHICS_OUTPUT_PROTOCOL_GUIDはグラフィックス機能のためのプロトコルを示すGUID
    let status = (efi_system_table.boot_services.locate_protocol)(
        &EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID,
        null_mut::<EfiVoid>(),
        &mut graphic_output_protocol as *mut *mut EfiGraphicsOutputProtocol as *mut *mut EfiVoid,   // UEFIとのやりとりをするために生ポインタにキャストしている
    );

    if status != EfiStatus::Success {
        return Err("Failed to locate graphics output protocol");
    }

    // 生ポインタから参照に変換して返す
    Ok(unsafe { &*graphic_output_protocol })
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
#[must_use]
#[repr(u64)]
enum EfiStatus {
    Success = 0,
    // Define other EFI status codes as needed
}

pub fn hlt() {
    unsafe {
        // CPUに停止させる命令
        asm!("hlt");
    }
}

// use core::{panic::PanicInfo, slice};

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        // 待機
        hlt();
    }
}

trait Bitmap {
    fn bytes_per_pixel(&self) -> i64;
    fn pixels_per_line(&self) -> i64;
    fn width(&self) -> i64;
    fn height(&self) -> i64;
    fn bur_mut(&self) -> *mut u8;

    unsafe fn unchecked_pixel_at_mut(&mut self, x: i64, y: i64) -> *mut u32 {
        self.bur_mut().add(
            ((y * self.pixels_per_line() + x) * self.bytes_per_pixel()) as usize,
        ) as *mut u32
    }

    fn pixel_at_mut(&mut self, x: i64, y: i64) -> Option<&mut u32> {
        
        if self.is_in_x_range(x) && self.is_in_y_range(y) {
            unsafe { Some(&mut *self.unchecked_pixel_at_mut(x, y)) }
        } else {
            None
        }
    }

    fn is_in_x_range(&self, x: i64) -> bool {
        0 <= x && x < min(self.width(), self.pixels_per_line())
    }
    fn is_in_y_range(&self, y: i64) -> bool {
        0 <= y && y < self.height()
    }
}

#[derive(Clone, Copy)]
struct VramBufferInfo {
    pub width: i64,
    pub height: i64,
    pub pixels_per_line: i64,
    pub buffer: *mut u8,
}

// BitmapトレイトをVramBufferInfo構造体に実装。bytes_per_pixelだけ4に固定
impl Bitmap for VramBufferInfo {
    fn bytes_per_pixel(&self) -> i64 {
        4
    }
    fn pixels_per_line(&self) -> i64 {
        self.pixels_per_line
    }
    fn width(&self) -> i64 {
        self.width
    }
    fn height(&self) -> i64 {
        self.height
    }
    fn bur_mut(&self) -> *mut u8 {
        self.buffer
    }
}

fn init_vram(efi_system_table: &EfiSystemTable) -> Result<VramBufferInfo> {
    
    let gp = locate_graphic_protolocol(efi_system_table)?;
    Ok(VramBufferInfo{
        width: gp.mode.info.horizontal_resolution as i64,
        height: gp.mode.info.vertical_resolution as i64,
        pixels_per_line: gp.mode.info.pixels_per_scan_line as i64,
        buffer: gp.mode.frame_buffer_base as *mut u8,
    })
}