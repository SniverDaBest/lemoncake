use core::{arch::asm, ptr};

use crate::{error, info, nftodo, pci::PCIDevice, sleep::Sleep};
use alloc::{string::String, vec::Vec};
use spin::Lazy;
use spinning_top::Spinlock;
use volatile::Volatile;
use x86_64::{
    VirtAddr,
    instructions::port::Port,
    structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB},
};

/// Busy
const ATA_SR_BSY: u8 = 0x80;
/// Drive Ready
const ATA_SR_DRDY: u8 = 0x40;
/// Drive Write Fault
const ATA_SR_DF: u8 = 0x20;
/// Drive Seek Complete
const ATA_SR_DSC: u8 = 0x10;
/// Data Request Ready
const ATA_SR_DRQ: u8 = 0x08;
/// Corrected Data
const ATA_SR_CORR: u8 = 0x04;
/// Index
const ATA_SR_IDX: u8 = 0x02;
/// Error
const ATA_SR_ERR: u8 = 0x01;

/// Bad Block
const ATA_ER_BBK: u8 = 0x80;
/// Uncorrectable Data
const ATA_ER_UNC: u8 = 0x40;
/// Media Changed
const ATA_ER_MC: u8 = 0x20;
/// ID Mark Not Found
const ATA_ER_IDNF: u8 = 0x10;
/// Media Change Request
const ATA_ER_MCR: u8 = 0x08;
/// Command Aborted
const ATA_ER_ABRT: u8 = 0x04;
/// Track 0 Not Found
const ATA_ER_TK0NF: u8 = 0x02;
/// No Address Mark
const ATA_ER_AMNF: u8 = 0x01;

const ATA_CMD_READ_PIO: u8 = 0x20;
const ATA_CMD_READ_PIO_EXT: u8 = 0x24;
const ATA_CMD_READ_DMA: u8 = 0xC8;
const ATA_CMD_READ_DMA_EXT: u8 = 0x25;
const ATA_CMD_WRITE_PIO: u8 = 0x30;
const ATA_CMD_WRITE_PIO_EXT: u8 = 0x34;
const ATA_CMD_WRITE_DMA: u8 = 0xCA;
const ATA_CMD_WRITE_DMA_EXT: u8 = 0x35;
const ATA_CMD_CACHE_FLUSH: u8 = 0xE7;
const ATA_CMD_CACHE_FLUSH_EXT: u8 = 0xEA;
const ATA_CMD_PACKET: u8 = 0xA0;
const ATA_CMD_IDENTIFY_PACKET: u8 = 0xA1;
const ATA_CMD_IDENTIFY: u8 = 0xEC;

const ATAPI_CMD_READ: u8 = 0xA8;
const ATAPI_CMD_EJECT: u8 = 0x1B;

const ATA_IDENT_DEVICETYPE: u8 = 0;
const ATA_IDENT_CYLINDERS: u8 = 2;
const ATA_IDENT_HEADS: u8 = 6;
const ATA_IDENT_SECTORS: u8 = 12;
const ATA_IDENT_SERIAL: u8 = 20;
const ATA_IDENT_MODEL: u8 = 54;
const ATA_IDENT_CAPABILITIES: u8 = 98;
const ATA_IDENT_FIELDVALID: u8 = 106;
const ATA_IDENT_MAX_LBA: u8 = 120;
const ATA_IDENT_COMMANDSETS: u8 = 164;
const ATA_IDENT_MAX_LBA_EXT: u8 = 200;

const IDE_ATA: u8 = 0x00;
const IDE_ATAPI: u8 = 0x01;

/* Parent-Child instead of Master-Slave.
Personally, I think it sounds better. */
const ATA_PARENT: u8 = 0x00;
const ATA_CHILD: u8 = 0x01;

const ATA_REG_DATA: u8 = 0x00;
const ATA_REG_ERROR: u8 = 0x01;
const ATA_REG_FEATURES: u8 = 0x01;
const ATA_REG_SECCOUNT0: u8 = 0x02;
const ATA_REG_LBA0: u8 = 0x03;
const ATA_REG_LBA1: u8 = 0x04;
const ATA_REG_LBA2: u8 = 0x05;
const ATA_REG_HDDEVSEL: u8 = 0x06;
const ATA_REG_COMMAND: u8 = 0x07;
const ATA_REG_STATUS: u8 = 0x07;
const ATA_REG_SECCOUNT1: u8 = 0x08;
const ATA_REG_LBA3: u8 = 0x09;
const ATA_REG_LBA4: u8 = 0x0A;
const ATA_REG_LBA5: u8 = 0x0B;
const ATA_REG_CONTROL: u8 = 0x0C;
const ATA_REG_ALTSTATUS: u8 = 0x0C;
const ATA_REG_DEVADDRESS: u8 = 0x0D;

const ATA_PRIMARY: u8 = 0x00;
const ATA_SECONDARY: u8 = 0x01;
const ATA_READ: u8 = 0x00;
const ATA_WRITE: u8 = 0x01;

#[repr(C)]
struct IDEChannelRegs {
    base: u16,
    ctrl: u16,
    bmide: u16,
    nien: u8,
}

static CHANNELS: Spinlock<Vec<IDEChannelRegs>> = Spinlock::new(Vec::new());
static IDE_BUF: Spinlock<[u8; 2048]> = Spinlock::new([0u8; 2048]);
static IDE_IRQ_INVOKED: Lazy<Spinlock<Volatile<u8>>> =
    Lazy::new(|| Spinlock::new(Volatile::new(0)));
static ATAPI_PACKET: Spinlock<[u8; 12]> = Spinlock::new([0xA8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

struct IDEDevice {
    rsv0: u8,
    channel: u8,
    drive: u8,
    typ: u16,
    sig: u16,
    capabilities: u16,
    cmdsets: u32,
    sz: u32,
    model: [u8; 41],
}

static IDE_DEVICES: Spinlock<Vec<IDEDevice>> = Spinlock::new(Vec::new());

unsafe fn ide_write(channel: u8, reg: u8, data: u8) {
    if reg > 0x07 && reg < 0x0C {
        ide_write(
            channel,
            ATA_REG_CONTROL,
            0x80 | CHANNELS.lock()[channel as usize].nien,
        );
    }

    if reg < 0x08 {
        Port::new(CHANNELS.lock()[channel as usize].base + reg as u16).write(data);
    } else if reg < 0x0C {
        Port::new(CHANNELS.lock()[channel as usize].base + reg as u16 - 0x06).write(data);
    } else if reg < 0x0E {
        Port::new(CHANNELS.lock()[channel as usize].base + reg as u16 - 0x0A).write(data);
    } else if reg < 0x16 {
        Port::new(CHANNELS.lock()[channel as usize].base + reg as u16 - 0x0E).write(data);
    }

    if reg > 0x07 && reg < 0x0C {
        ide_write(
            channel,
            ATA_REG_CONTROL,
            CHANNELS.lock()[channel as usize].nien,
        );
    }
}

unsafe fn ide_read(channel: u8, reg: u8) -> u8 {
    if reg > 0x07 && reg < 0x0C {
        ide_write(
            channel,
            ATA_REG_CONTROL,
            0x80 | CHANNELS.lock()[channel as usize].nien,
        );
    }

    if reg < 0x08 {
        return Port::new(CHANNELS.lock()[channel as usize].base + reg as u16).read();
    } else if reg < 0x0C {
        return Port::new(CHANNELS.lock()[channel as usize].base + reg as u16 - 0x06).read();
    } else if reg < 0x0E {
        return Port::new(CHANNELS.lock()[channel as usize].base + reg as u16 - 0x0A).read();
    } else if reg < 0x16 {
        return Port::new(CHANNELS.lock()[channel as usize].base + reg as u16 - 0x0E).read();
    }

    if reg > 0x07 && reg < 0x0C {
        ide_write(
            channel,
            ATA_REG_CONTROL,
            CHANNELS.lock()[channel as usize].nien,
        );
    }

    return 0;
}

unsafe fn ide_read_buffer(channel: u8, reg: u8, buffer: &mut [u32], quads: u32) {
    if reg > 0x07 && reg < 0x0C {
        ide_write(
            channel,
            ATA_REG_CONTROL,
            0x80 | CHANNELS.lock()[channel as usize].nien,
        );
    }

    let port_addr = if reg < 0x08 {
        CHANNELS.lock()[channel as usize].base + reg as u16 - 0x00
    } else if reg < 0x0C {
        CHANNELS.lock()[channel as usize].base + reg as u16 - 0x06
    } else if reg < 0x0E {
        CHANNELS.lock()[channel as usize].ctrl + reg as u16 - 0x0A
    } else if reg < 0x16 {
        CHANNELS.lock()[channel as usize].bmide + reg as u16 - 0x0E
    } else {
        error!("(IDE) Invalid register address!");
        return;
    };

    let mut p = Port::new(port_addr);
    for i in 0..quads {
        buffer[i as usize] = p.read();
    }

    if reg > 0x07 && reg < 0x0C {
        ide_write(
            channel,
            ATA_REG_CONTROL,
            CHANNELS.lock()[channel as usize].nien,
        )
    }
}

unsafe fn ide_polling(channel: u8, advanced_check: u32) -> u8 {
    for _ in 0..4 {
        ide_read(channel, ATA_REG_ALTSTATUS);
    }

    while ide_read(channel, ATA_REG_STATUS) & ATA_SR_BSY != 0 { /* until not busy */ }

    if advanced_check != 0 {
        let state = ide_read(channel, ATA_REG_STATUS);

        if state & ATA_SR_ERR != 0 {
            return 2;
        }

        if state & ATA_SR_DF != 0 {
            return 1;
        }

        if state & ATA_SR_DRQ == 0 {
            return 3;
        }
    }

    return 0;
}

unsafe fn fmt_error(drive: u32, err: u8) -> u8 {
    if err == 0 {
        return err;
    }

    let mut ret = 0u8;

    if err == 1 {
        error!("(IDE) Device fault");
        ret = 19;
    } else if err == 2 {
        let st = ide_read(IDE_DEVICES.lock()[drive as usize].channel, ATA_REG_ERROR);
        if st & ATA_ER_AMNF != 0 {
            error!("(IDE) No Address Mark Found");
            ret = 7;
        }
        if st & ATA_ER_TK0NF != 0 {
            error!("(IDE) No Media or Media Error");
            ret = 3;
        }
        if st & ATA_ER_ABRT != 0 {
            error!("(IDE) Command Aborted");
            ret = 20;
        }
        if st & ATA_ER_MCR != 0 {
            error!("(IDE) No Media or Media Error");
            ret = 3;
        }
        if st & ATA_ER_IDNF != 0 {
            error!("(IDE) ID mark not Found");
            ret = 21;
        }
        if st & ATA_ER_MC != 0 {
            error!("(IDE) No Media or Media Error");
            ret = 3;
        }
        if st & ATA_ER_UNC != 0 {
            error!("(IDE) Uncorrectable Data Error");
            ret = 22;
        }
        if st & ATA_ER_BBK != 0 {
            error!("(IDE) Bad Sectors");
            ret = 13;
        }
    } else if err == 3 {
        error!("(IDE) Reads Nothing");
        ret = 23;
    } else if err == 4 {
        error!("(IDE) Write Protected");
        ret = 8;
    }

    return ret;
}

fn name_as_string(n: [u8; 41]) -> String {
    let mut cs: [char; 41] = ['\0'; 41];

    for (i, c) in n.iter().enumerate() {
        cs[i] = *c as char;
    }

    return cs.into_iter().collect::<String>();
}

unsafe fn ide_cache_flush(channel: u8, lba_mode: u8) -> bool {
    ide_write(
        channel,
        ATA_REG_COMMAND,
        if lba_mode == 2 {
            ATA_CMD_CACHE_FLUSH_EXT
        } else {
            ATA_CMD_CACHE_FLUSH
        },
    );

    let mut timeout = 100000;
    while timeout > 0 {
        let stat = ide_read(channel, ATA_REG_STATUS);
        if stat & ATA_SR_BSY == 0 {
            return true;
        }
        timeout -= 1;
    }

    return false;
}

pub unsafe fn init_ide(
    dev: PCIDevice,
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    let frame = frame_allocator
        .allocate_frame()
        .expect("(IDE) Unable to init frame!");
    mapper.map_to(
        Page::containing_address(VirtAddr::new(dev.bar_address(4).unwrap())),
        frame,
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
        frame_allocator,
    ).expect("(IDE) Unable to map BAR4!").flush();

    let mut count = 0;
    let mut typ = 0u8;

    let mut channels = CHANNELS.lock();

    channels.push(IDEChannelRegs {
        base: 0,
        ctrl: 0,
        bmide: 0,
        nien: 0,
    });
    channels.push(IDEChannelRegs {
        base: 0,
        ctrl: 0,
        bmide: 0,
        nien: 0,
    });

    channels[ATA_PRIMARY as usize].base = 0x1F0;
    channels[ATA_PRIMARY as usize].ctrl = 0x3F6;
    channels[ATA_SECONDARY as usize].base = 0x170;
    channels[ATA_SECONDARY as usize].ctrl = 0x376;

    let bar4 = dev.read_bar(4).unwrap().0 & 0xFFFFFFFC;
    channels[ATA_PRIMARY as usize].bmide = bar4 as u16;
    channels[ATA_SECONDARY as usize].bmide = (bar4 as u16).wrapping_add(8);

    ide_write(ATA_PRIMARY, ATA_REG_CONTROL, 2);
    ide_write(ATA_SECONDARY, ATA_REG_CONTROL, 2);

    for i in 0..2 {
        for j in 0..2 {
            let mut err = 0u8;
            let mut status: u8;

            IDE_DEVICES.lock()[count as usize].rsv0 = 0;

            ide_write(i, ATA_REG_HDDEVSEL, 0xA0 | (j << 4));
            Sleep::ms(1);

            ide_write(i, ATA_REG_COMMAND, ATA_CMD_IDENTIFY);
            Sleep::ms(1);

            if ide_read(i, ATA_REG_STATUS) == 0 {
                continue;
            }

            loop {
                status = ide_read(i, ATA_REG_STATUS);
                if status & ATA_SR_ERR != 0 {
                    err = 1;
                    break;
                }
                if !status & ATA_SR_BSY != 0 && status & ATA_SR_DRQ != 0 {
                    break;
                }
            }

            if err != 0 {
                let cl = ide_read(i, ATA_REG_LBA1);
                let ch = ide_read(i, ATA_REG_LBA2);

                if cl == 0x14 && ch == 0xEB {
                    typ = IDE_ATAPI;
                } else if cl == 0x69 && ch == 0x96 {
                    typ = IDE_ATAPI;
                } else {
                    continue;
                }

                ide_write(i, ATA_REG_COMMAND, ATA_CMD_IDENTIFY_PACKET);
                Sleep::ms(1);
            }

            ide_read_buffer(
                i,
                ATA_REG_DATA,
                core::slice::from_raw_parts_mut(
                    IDE_BUF.lock().as_mut_ptr() as *mut u32,
                    IDE_BUF.lock().len() / 4,
                ),
                128,
            );

            IDE_DEVICES.lock()[count].rsv0 = 1;
            IDE_DEVICES.lock()[count].typ = typ as u16;
            IDE_DEVICES.lock()[count].channel = i;
            IDE_DEVICES.lock()[count].drive = j;
            IDE_DEVICES.lock()[count].sig = ptr::read_unaligned(
                IDE_BUF.lock().as_ptr().add(ATA_IDENT_DEVICETYPE as usize) as *const u16,
            );
            IDE_DEVICES.lock()[count].capabilities = ptr::read_unaligned(
                IDE_BUF.lock().as_ptr().add(ATA_IDENT_CAPABILITIES as usize) as *const u16,
            );
            IDE_DEVICES.lock()[count].cmdsets = ptr::read_unaligned(
                IDE_BUF.lock().as_ptr().add(ATA_IDENT_COMMANDSETS as usize) as *const u32,
            );

            if IDE_DEVICES.lock()[count].cmdsets & (1 << 26) != 0 {
                IDE_DEVICES.lock()[count].sz = ptr::read_unaligned(
                    IDE_BUF.lock().as_ptr().add(ATA_IDENT_MAX_LBA_EXT as usize) as *const u32,
                );
            } else {
                IDE_DEVICES.lock()[count].sz = ptr::read_unaligned(
                    IDE_BUF.lock().as_ptr().add(ATA_IDENT_MAX_LBA as usize) as *const u32,
                );
            }

            for k in 0..20 {
                let kk = k * 2;
                IDE_DEVICES.lock()[count].model[kk] =
                    IDE_BUF.lock()[(ATA_IDENT_MODEL + kk as u8 + 1) as usize];
                IDE_DEVICES.lock()[count].model[kk + 1] =
                    IDE_BUF.lock()[(ATA_IDENT_MODEL + kk as u8) as usize];
            }

            IDE_DEVICES.lock()[count].model[40] = 0;

            count += 1;
        }
    }

    for i in 0..4 {
        if IDE_DEVICES.lock()[i].rsv0 == 1 {
            info!(
                "(ATA) Found {} drive! Size: {}GB, Model: {}",
                if IDE_DEVICES.lock()[i].typ == 0 {
                    "ATA"
                } else {
                    "ATAPI"
                },
                IDE_DEVICES.lock()[i].sz / 1024 / 1024 / 2,
                name_as_string(IDE_DEVICES.lock()[i].model)
            );
        }
    }
}

pub unsafe fn ide_ata_access(
    direction: u8,
    drive: u8,
    lba: u32,
    numsects: u8,
    #[allow(unused)] selector: u16,
    mut edi: u32,
) -> u8 {
    let lba_mode: u8;
    #[allow(unused_mut)]
    let mut dma: u8 = 0;
    let mut cmd: u8 = 0;
    let mut lba_io = [0u8; 6];
    let channel = IDE_DEVICES.lock()[drive as usize].channel as u32;
    let childbit = IDE_DEVICES.lock()[drive as usize].drive as u32;
    let bus = CHANNELS.lock()[channel as usize].base as u32;
    let words: u32 = 256;
    let cyl: u16;
    let head: u8;
    let sect: u8;
    let err: u8 = 0;

    IDE_IRQ_INVOKED.lock().write(0);
    CHANNELS.lock()[channel as usize].nien = IDE_IRQ_INVOKED.lock().read() + 0x02;
    ide_write(
        channel as u8,
        ATA_REG_CONTROL,
        IDE_IRQ_INVOKED.lock().read() + 0x02,
    );

    if lba >= 0x10000000 {
        lba_mode = 2;
        lba_io[0] = ((lba & 0x000000FF) >> 0) as u8;
        lba_io[1] = ((lba & 0x0000FF00) >> 8) as u8;
        lba_io[2] = ((lba & 0x00FF0000) >> 16) as u8;
        lba_io[3] = ((lba & 0xFF000000) >> 24) as u8;
        lba_io[4] = 0;
        lba_io[5] = 0;
        head = 0;
    } else if IDE_DEVICES.lock()[drive as usize].capabilities & 0x200 != 0 {
        lba_mode = 1;
        lba_io[0] = ((lba & 0x00000FF) >> 0) as u8;
        lba_io[1] = ((lba & 0x000FF00) >> 8) as u8;
        lba_io[2] = ((lba & 0x0FF0000) >> 16) as u8;
        lba_io[3] = 0;
        lba_io[4] = 0;
        lba_io[5] = 0;
        head = ((lba & 0xF000000) >> 24) as u8;
    } else {
        lba_mode = 0;
        sect = ((lba % 63) + 1) as u8;
        cyl = ((lba + 1 - sect as u32) / (16 * 63)) as u16;
        lba_io[0] = sect;
        lba_io[1] = ((cyl >> 0) & 0xFF) as u8;
        lba_io[2] = ((cyl >> 8) & 0xFF) as u8;
        lba_io[3] = 0;
        lba_io[4] = 0;
        lba_io[5] = 0;
        head = ((lba + 1 - sect as u32) % (16 * 63) / (63)) as u8;
    }

    while ide_read(channel as u8, ATA_REG_STATUS) & ATA_SR_BSY != 0 { /* Wait until not busy */ }

    ide_write(
        channel as u8,
        ATA_REG_HDDEVSEL,
        if lba_mode == 0 { 0xA0 } else { 0xE0 } | ((childbit as u8) << 4) | head,
    );

    if lba_mode == 2 {
        ide_write(channel as u8, ATA_REG_SECCOUNT1, 0);
        ide_write(channel as u8, ATA_REG_LBA3, lba_io[3]);
        ide_write(channel as u8, ATA_REG_LBA4, lba_io[4]);
        ide_write(channel as u8, ATA_REG_LBA5, lba_io[5]);
    }

    ide_write(channel as u8, ATA_REG_SECCOUNT0, numsects);
    ide_write(channel as u8, ATA_REG_LBA0, lba_io[0]);
    ide_write(channel as u8, ATA_REG_LBA1, lba_io[1]);
    ide_write(channel as u8, ATA_REG_LBA2, lba_io[2]);

    if lba_mode == 0 && dma == 0 && direction == 0 {
        cmd = ATA_CMD_READ_PIO
    };
    if lba_mode == 1 && dma == 0 && direction == 0 {
        cmd = ATA_CMD_READ_PIO
    };
    if lba_mode == 2 && dma == 0 && direction == 0 {
        cmd = ATA_CMD_READ_PIO_EXT
    };
    if lba_mode == 0 && dma == 1 && direction == 0 {
        cmd = ATA_CMD_READ_DMA
    };
    if lba_mode == 1 && dma == 1 && direction == 0 {
        cmd = ATA_CMD_READ_DMA
    };
    if lba_mode == 2 && dma == 1 && direction == 0 {
        cmd = ATA_CMD_READ_DMA_EXT
    };
    if lba_mode == 0 && dma == 0 && direction == 1 {
        cmd = ATA_CMD_WRITE_PIO
    };
    if lba_mode == 1 && dma == 0 && direction == 1 {
        cmd = ATA_CMD_WRITE_PIO
    };
    if lba_mode == 2 && dma == 0 && direction == 1 {
        cmd = ATA_CMD_WRITE_PIO_EXT
    };
    if lba_mode == 0 && dma == 1 && direction == 1 {
        cmd = ATA_CMD_WRITE_DMA
    };
    if lba_mode == 1 && dma == 1 && direction == 1 {
        cmd = ATA_CMD_WRITE_DMA
    };
    if lba_mode == 2 && dma == 1 && direction == 1 {
        cmd = ATA_CMD_WRITE_DMA_EXT
    };
    ide_write(channel as u8, ATA_REG_COMMAND, cmd);

    if dma != 0 {
        if direction == 0 {
            // dma read
            nftodo!("(IDE) ide_ata_access - GOTO: // dma read");
        } else {
            // dma write
            nftodo!("(IDE) ide_ata_access - GOTO: // dma write");
        }
    } else {
        if direction == 0 {
            for _ in 0..numsects {
                if err == ide_polling(channel as u8, 1) {
                    return err;
                }

                asm!(
                    "rep insw",
                    in("ecx") words,
                    in("dx") bus,
                    inout("edi") edi => _,
                    options(preserves_flags, nostack),
                );

                edi += words * 2;
            }
        } else {
            for _ in 0..numsects {
                ide_polling(channel as u8, 0);
                asm!(
                    "rep outsw",
                    in("ecx") words,
                    in("dx") bus,
                    in("esi") edi,
                    options(preserves_flags, nostack),
                );
                edi += words * 2;
            }

            if !ide_cache_flush(channel as u8, lba_mode) {
                error!("(IDE) Timed out while flushing cache!");
                return u8::MAX;
            }

            ide_polling(channel as u8, 0);
        }
    }

    return 0;
}

fn ide_wait_irq() {
    while !IDE_IRQ_INVOKED.lock().read() != 0 { /* Wait for IRQ */ }
    IDE_IRQ_INVOKED.lock().write(0);
}

fn ide_irq() {
    IDE_IRQ_INVOKED.lock().write(1);
}

pub unsafe fn ide_atapi_read(
    drive: u8,
    lba: u32,
    numsects: u8,
    #[allow(unused_variables)] selector: u16,
    mut edi: u32,
) -> u8 {
    let channel = IDE_DEVICES.lock()[drive as usize].channel;
    let childbit = IDE_DEVICES.lock()[drive as usize].drive;
    let bus = CHANNELS.lock()[channel as usize].base;
    let words = 1024u16;
    let mut err;

    IDE_IRQ_INVOKED.lock().write(0);
    if let Some(cr) = CHANNELS.lock().get_mut(channel as usize) {
        cr.nien = 0;
        ide_write(channel, ATA_REG_CONTROL, cr.nien);
    }

    let mut packet = ATAPI_PACKET.lock();

    packet[0] = ATAPI_CMD_READ;
    packet[1] = 0x0;
    packet[2] = ((lba >> 26) & 0xFF) as u8;
    packet[3] = ((lba >> 16) & 0xFF) as u8;
    packet[4] = ((lba >> 8) & 0xFF) as u8;
    packet[5] = ((lba >> 0) & 0xFF) as u8;
    packet[6] = 0x0;
    packet[7] = 0x0;
    packet[8] = 0x0;
    packet[9] = numsects;
    packet[10] = 0x0;
    packet[11] = 0x0;

    ide_write(channel, ATA_REG_HDDEVSEL, childbit << 4);

    for _ in 0..4 {
        ide_read(channel, ATA_REG_ALTSTATUS);
    }

    ide_write(channel, ATA_REG_FEATURES, 0);

    ide_write(channel, ATA_REG_LBA1, ((words * 2) & 0xFF) as u8);
    ide_write(channel, ATA_REG_LBA2, ((words * 2) >> 8) as u8);

    ide_write(channel, ATA_REG_COMMAND, ATA_CMD_PACKET);

    err = ide_polling(channel, 1);

    if err != 0 {
        return err;
    }

    asm!(
        "rep outsw",
        in("ecx") 6,
        in("dx") bus,
        in("esi") ATAPI_PACKET.data_ptr() as u32,
        options(preserves_flags, nostack),
    );

    for _ in 0..numsects {
        ide_wait_irq();
        err = ide_polling(channel, 1);
        if err != 0 {
            return err;
        }

        asm!(
            "rep insw",
            in("ecx") words,
            in("dx") bus,
            in("esi") edi,
        );

        edi += words as u32 * 2;
    }

    ide_wait_irq();

    while ide_read(channel, ATA_REG_STATUS) & (ATA_SR_BSY | ATA_SR_DRQ) != 0 { /* Wait to no longer be busy. */
    }

    return 0;
}

pub unsafe fn ide_read_sectors(drive: u8, numsects: u8, lba: u32, es: u16, edi: u32) -> u8 {
    if drive > 3 || IDE_DEVICES.lock()[drive as usize].rsv0 == 0 {
        return 0x1;
    } else if (lba + numsects as u32) > IDE_DEVICES.lock()[drive as usize].sz
        && IDE_DEVICES.lock()[drive as usize].typ == IDE_ATA as u16
    {
        return 0x2;
    } else {
        let mut err: u8 = 0;
        if IDE_DEVICES.lock()[drive as usize].typ == IDE_ATA as u16 {
            err = ide_ata_access(ATA_READ, drive, lba, numsects, es, edi);
        } else if IDE_DEVICES.lock()[drive as usize].typ == IDE_ATAPI as u16 {
            for i in 0..numsects {
                err = ide_atapi_read(drive, lba + i as u32, 1, es, edi + (i as u32 * 2048));
            }
        }

        return fmt_error(drive as u32, err);
    }
}

pub unsafe fn ide_write_sectors(drive: u8, numsects: u8, lba: u32, es: u16, edi: u32) -> u8 {
    if drive > 3 || IDE_DEVICES.lock()[drive as usize].rsv0 == 0 {
        return 0x1;
    } else if lba + numsects as u32 > IDE_DEVICES.lock()[drive as usize].sz
        && IDE_DEVICES.lock()[drive as usize].typ == IDE_ATA as u16
    {
        return 0x2;
    } else {
        let mut err = 0u8;

        if IDE_DEVICES.lock()[drive as usize].typ == IDE_ATA as u16 {
            err = ide_ata_access(ATA_READ, drive, lba, numsects, es, edi);
        } else if IDE_DEVICES.lock()[drive as usize].typ == IDE_ATAPI as u16 {
            for i in 0..numsects {
                err = ide_atapi_read(drive, lba + i as u32, 1, es, edi + (i as u32 * 2048));
            }
        }
        return fmt_error(drive as u32, err);
    }
}

pub unsafe fn ide_atapi_eject(drive: u8) -> u8 {
    let channel = IDE_DEVICES.lock()[drive as usize].channel as u32;
    let childbit = IDE_DEVICES.lock()[drive as usize].drive as u32;
    let bus = CHANNELS.lock()[channel as usize].base as u32;
    let mut err;
    IDE_IRQ_INVOKED.lock().write(0);

    if drive > 3 || IDE_DEVICES.lock()[drive as usize].rsv0 == 0 {
        return 0x1;
    } else if IDE_DEVICES.lock()[drive as usize].typ == IDE_ATA as u16 {
        return 20;
    } else {
        IDE_IRQ_INVOKED.lock().write(0);
        CHANNELS.lock()[channel as usize].nien = IDE_IRQ_INVOKED.lock().read();
        ide_write(
            channel as u8,
            ATA_REG_CONTROL,
            CHANNELS.lock()[channel as usize].nien,
        );

        let mut packet = ATAPI_PACKET.lock();
        packet[0] = ATAPI_CMD_EJECT;
        packet[1] = 0x0;
        packet[2] = 0x0;
        packet[3] = 0x0;
        packet[4] = 0x0;
        packet[5] = 0x0;
        packet[6] = 0x0;
        packet[7] = 0x0;
        packet[8] = 0x0;
        packet[9] = 0x0;
        packet[10] = 0x0;
        packet[11] = 0x0;

        ide_write(channel as u8, ATA_REG_HDDEVSEL, (childbit as u8) << 4);

        for _ in 0..4 {
            ide_read(channel as u8, ATA_REG_ALTSTATUS);
        }

        ide_write(channel as u8, ATA_REG_COMMAND, ATA_CMD_PACKET);

        err = ide_polling(channel as u8, 1);

        if err == 0 {
            asm!(
                "rep outsw",
                in("ecx") 6,
                in("dx") bus,
                in("esi") ATAPI_PACKET.data_ptr() as u32,
                options(preserves_flags, nostack),
            );

            ide_wait_irq();
            err = ide_polling(channel as u8, 1);
            if err == 3 {
                err = 0;
            }
        }

        return fmt_error(drive as u32, err);
    }
}
