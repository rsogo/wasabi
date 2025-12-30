#![no_std]
#![no_main]
#![feature(offset_of)]

use core::arch::asm;
use core::mem::offset_of;
use core::mem::size_of;
use core::panic::PanicInfo;
use core::ptr::null_mut;
use core::slice;

type EfiVoid = u8;
type EfiHandle = u64;
type Result<T> = core::result::Result<T, &'static str>;

// no_mangleを指定することで、コンパイル時の名前の変更を防ぐ。
// UEFIのエントリポイント
// _image_handle: UEFIのイメージハンドル
// efi_system_table: UEFIのシステムテーブルへのポインタ
#[no_mangle]
fn efi_main(_image_handle: EfiHandle, efi_system_table: &EfiSystemTable) -> ! {
    let efi_graphics_output_protocol = locate_graphic_protolocol(efi_system_table).unwrap();
    let vram_address = efi_graphics_output_protocol.mode.frame_buffer_base;
    let vram_byte_size = efi_graphics_output_protocol.mode.frame_buffer_size;
    let vram = unsafe {
        slice::from_raw_parts_mut(
            vram_address as *mut u32,
            vram_byte_size as usize / size_of::<u32>(),
        )
    };

    for e in vram {
        *e = 0xFFFFFF;
    }

    loop {
        // 待機
        hlt();
    }
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
