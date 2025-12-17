// SPDX-License-Identifier: GPL-2.0

//! The Scull device from LDD3, reimplemented in Rust.

use kernel::{
    c_str,
    fs::{file, File, Kiocb},
    ioctl::{_IOC_NR, _IOC_SIZE, _IOC_TYPE, _IOR},
    iov::{IovIterDest, IovIterSource},
    kvec,
    miscdevice::{MiscDevice, MiscDeviceOptions, MiscDeviceRegistration},
    new_mutex,
    prelude::*,
    sync::Mutex,
    types::ForeignOwnable,
    uaccess::UserSlice,
};

module! {
    type: ScullDeviceModule,
    name: "Scull",
    authors: ["Emmanuel Amoah"],
    description: "The Scull device from LDD3, reimplemented in Rust.",
    license: "GPL",
}

const SCULL_QUANTUM: u32 = 4000;
const SCULL_QSET: u32 = 1000;

// Ioctl definitions

// Use '`' as magic number
const SCULL_IOC_MAGIC: u32 = '`' as u32;
// Please use a different 8-bit number in your code

const SCULL_IOCGQUANTUM: u32 = _IOR::<i32>(SCULL_IOC_MAGIC, 5);
const SCULL_IOCGQSET: u32 = _IOR::<i32>(SCULL_IOC_MAGIC, 6);

const SCULL_IOC_MAXNR: u32 = 6;

#[pin_data]
struct ScullDeviceModule {
    #[pin]
    _miscdev_reg: MiscDeviceRegistration<ScullDevice>,
}

impl kernel::InPlaceModule for ScullDeviceModule {
    fn init(_module: &'static ThisModule) -> impl PinInit<Self, Error> {
        let options = MiscDeviceOptions {
            name: c_str!("scull"),
        };

        try_pin_init!(ScullDeviceModule {
            _miscdev_reg <- MiscDeviceRegistration::register(options),
        })
    }
}

struct ScullQset {
    data: Option<KVec<Option<KBox<KVec<u8>>>>>,
    next: Option<KBox<ScullQset>>,
}

impl ScullQset {
    fn new() -> Self {
        ScullQset {
            data: None,
            next: None,
        }
    }
}

struct ScullDevice {
    data: Option<KBox<ScullQset>>,
    qset: u32,
    size: usize,
    quantum: u32,
}

impl ScullDevice {
    // Empty out the scull device; must be called with the device
    // mutex locked.
    fn trim(&mut self) {
        if let Some(data) = self.data.take() {
            drop(data);
        };
        self.size = 0;
    }

    // Follow the list
    fn follow(&mut self, mut n: usize) -> Option<&mut KBox<ScullQset>> {
        let qs = &mut self.data;

        // Allocate first qset explicitly if need be
        if qs.is_none() {
            *qs = Some(KBox::new(ScullQset::new(), GFP_KERNEL).ok()?);
        }

        let mut qs = qs.as_mut()?;

        // Follow the list
        while n > 0 {
            n -= 1;
            if qs.next.is_none() {
                qs.next = Some(KBox::new(ScullQset::new(), GFP_KERNEL).ok()?);
            }
            qs = qs.next.as_mut()?;
        }

        Some(qs)
    }
}

#[vtable]
impl MiscDevice for ScullDevice {
    type Ptr = Pin<KBox<Mutex<Self>>>;

    fn open(file: &File, _misc: &MiscDeviceRegistration<Self>) -> Result<Self::Ptr> {
        let dev = KBox::pin_init(
            new_mutex!(ScullDevice {
                data: None,
                qset: SCULL_QSET,
                size: 0,
                quantum: SCULL_QUANTUM,
            }),
            GFP_KERNEL,
        )?;

        // Now trim to 0 the length of the device if open was write-only
        // (currently redundant)
        if (file.flags() & file::flags::O_ACCMODE) == file::flags::O_WRONLY {
            let mut dev = dev.lock();
            dev.trim();
        }

        Ok(dev)
    }

    // Data management: read and write

    fn read_iter(mut kiocb: Kiocb<'_, Self::Ptr>, iov: &mut IovIterDest<'_>) -> Result<usize> {
        let f_pos = kiocb.ki_pos();
        let mut count = iov.len();

        let dev = kiocb.file();
        let mut dev = dev.lock();

        let dptr: Option<&mut KBox<ScullQset>>; // The first listitem
        let quantum = dev.quantum as usize;
        let qset = dev.qset as usize;
        let itemsize = quantum * qset; // How many bytes in the listitem
        let (item, s_pos, q_pos, rest);
        let retval = 0usize;

        if f_pos >= dev.size as i64 {
            return Ok(retval);
        }

        // Find listitem, qset index, and offset in the quantum
        item = (f_pos / itemsize as i64) as usize;
        rest = (f_pos % itemsize as i64) as usize;
        s_pos = rest / quantum;
        q_pos = rest % quantum;

        // Follow the list up to the right position
        dptr = dev.follow(item);

        let Some(quantum_vec) = dptr
            .and_then(|dptr| dptr.data.as_ref())
            .and_then(|data| data[s_pos].as_ref())
        else {
            return Ok(retval);
        };

        // Read only up to the end of this quantum
        if count > quantum - q_pos {
            count = quantum - q_pos;
        }

        let retval =
            iov.simple_read_from_buffer(kiocb.ki_pos_mut(), &quantum_vec[q_pos..q_pos + count])?;

        Ok(retval)
    }

    fn write_iter(mut kiocb: Kiocb<'_, Self::Ptr>, iov: &mut IovIterSource<'_>) -> Result<usize> {
        let f_pos = kiocb.ki_pos();
        let mut count = iov.len();

        let dev = kiocb.file();
        let mut dev = dev.lock();

        let dptr: Option<&mut KBox<ScullQset>>; // The first listitem
        let quantum = dev.quantum as usize;
        let qset = dev.qset as usize;
        let itemsize = quantum * qset; // How many bytes in the listitem
        let (item, s_pos, q_pos, rest);
        let retval = ENOMEM; // Value used for returned error

        // Find listitem, qset index and offset in the quantum
        item = (f_pos / itemsize as i64) as usize;
        rest = (f_pos % itemsize as i64) as usize;
        s_pos = rest / quantum;
        q_pos = rest % quantum;

        // Follow the list up to the right position
        dptr = dev.follow(item);

        let dptr = dptr.ok_or(retval)?;
        let data = &mut dptr.data;

        if data.is_none() {
            *data = Some(KVec::with_capacity(qset, GFP_KERNEL)?);
            if let Some(data) = data {
                for _i in 0..qset {
                    data.push_within_capacity(None)?;
                }
            }
        }
        let data = data.as_mut().ok_or(retval)?;

        if data[s_pos].is_none() {
            data[s_pos] = Some(KBox::new(kvec![0; quantum]?, GFP_KERNEL)?);
        }

        let quantum_vec = data[s_pos].as_mut().ok_or(retval)?;

        // Write only up to the end of this quantum
        if count > quantum - q_pos {
            count = quantum - q_pos;
        }

        let retval = iov.copy_from_iter(&mut quantum_vec[q_pos..q_pos + count]);
        *kiocb.ki_pos_mut() += count as i64;

        // Update the size
        if dev.size < kiocb.ki_pos() as usize {
            dev.size = kiocb.ki_pos() as usize
        }

        Ok(retval)
    }

    // The ioctl() implementation

    fn ioctl(
        dev: <Self::Ptr as ForeignOwnable>::Borrowed<'_>,
        _file: &File,
        cmd: u32,
        arg: usize,
    ) -> Result<isize> {
        let arg = UserPtr::from_addr(arg);
        let size = _IOC_SIZE(cmd);

        // Extract the type and number bitfields, and don't decode
        // wrong cmds: return ENOTTY (inappropriate ioctl)
        if _IOC_TYPE(cmd) != SCULL_IOC_MAGIC {
            return Err(ENOTTY);
        }
        if _IOC_NR(cmd) > SCULL_IOC_MAXNR {
            return Err(ENOTTY);
        }

        let mut writer = UserSlice::new(arg, size).writer();

        let dev = dev.lock();
        match cmd {
            SCULL_IOCGQUANTUM => {
                writer.write(&dev.quantum)?;
            }
            SCULL_IOCGQSET => {
                writer.write(&dev.qset)?;
            }
            _ => return Err(ENOTTY),
        }

        Ok(0)
    }
}
