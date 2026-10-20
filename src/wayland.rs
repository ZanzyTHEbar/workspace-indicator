use std::env;
use std::ffi::CString;
use std::io::{self, ErrorKind, Read, Write};
use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::ptr;
use std::slice;

use crate::config::Position;
use crate::ffi;
use crate::ipc::MonitorTarget;

const WL_DISPLAY_ID: u32 = 1;

// wl_display requests
const WL_DISPLAY_SYNC: u16 = 0;
const WL_DISPLAY_GET_REGISTRY: u16 = 1;

// wl_display events
const WL_DISPLAY_ERROR: u16 = 0;

// wl_registry requests & events
const WL_REGISTRY_BIND: u16 = 0;
const WL_REGISTRY_GLOBAL: u16 = 0;
const WL_REGISTRY_GLOBAL_REMOVE: u16 = 1;

// wl_callback events
const WL_CALLBACK_DONE: u16 = 0;

// wl_compositor requests
const WL_COMPOSITOR_CREATE_SURFACE: u16 = 0;
const WL_COMPOSITOR_CREATE_REGION: u16 = 1;

// wl_surface requests
const WL_SURFACE_DESTROY: u16 = 0;
const WL_SURFACE_ATTACH: u16 = 1;
const WL_SURFACE_DAMAGE: u16 = 2;
const WL_SURFACE_SET_INPUT_REGION: u16 = 5;
const WL_SURFACE_COMMIT: u16 = 6;
const WL_SURFACE_DAMAGE_BUFFER: u16 = 9;

// wl_shm requests & formats
const WL_SHM_CREATE_POOL: u16 = 0;
const WL_SHM_FORMAT_ARGB8888: u32 = 0;

// wl_shm_pool requests
const WL_SHM_POOL_CREATE_BUFFER: u16 = 0;
const WL_SHM_POOL_RESIZE: u16 = 2;

// wl_buffer requests & events
const WL_BUFFER_DESTROY: u16 = 0;
const WL_BUFFER_RELEASE: u16 = 0;

// wl_output events
const WL_OUTPUT_GEOMETRY: u16 = 0;
const WL_OUTPUT_SCALE: u16 = 3;
const WL_OUTPUT_NAME: u16 = 4;

// zwlr_layer_shell_v1 requests & constants
const ZWLR_LAYER_SHELL_GET_LAYER_SURFACE: u16 = 0;
const ZWLR_LAYER_SHELL_LAYER_OVERLAY: u32 = 3;

// zwlr_layer_surface_v1 requests & events
const ZWLR_LAYER_SURFACE_SET_SIZE: u16 = 0;
const ZWLR_LAYER_SURFACE_SET_ANCHOR: u16 = 1;
const ZWLR_LAYER_SURFACE_SET_EXCLUSIVE_ZONE: u16 = 2;
const ZWLR_LAYER_SURFACE_SET_MARGIN: u16 = 3;
const ZWLR_LAYER_SURFACE_SET_KEYBOARD_INTERACTIVITY: u16 = 4;
const ZWLR_LAYER_SURFACE_ACK_CONFIGURE: u16 = 6;
const ZWLR_LAYER_SURFACE_DESTROY: u16 = 7;

const ZWLR_LAYER_SURFACE_CONFIGURE: u16 = 0;
const ZWLR_LAYER_SURFACE_CLOSED: u16 = 1;

const ANCHOR_TOP: u32 = 1;
const ANCHOR_BOTTOM: u32 = 2;
const ANCHOR_LEFT: u32 = 4;
const ANCHOR_RIGHT: u32 = 8;

#[derive(Clone, Debug, Default)]
pub struct WaylandOutput {
    pub global_name: u32,
    pub obj_id: u32,
    pub name: String,
    pub make: String,
    pub model: String,
    pub x: i32,
    pub y: i32,
    pub scale: i32,
}

struct ShmPool {
    fd: RawFd,
    pool_id: u32,
    ptr: *mut u8,
    capacity: usize,
    slot_idx: usize,
}

impl Drop for ShmPool {
    fn drop(&mut self) {
        unsafe {
            if !self.ptr.is_null() && self.capacity > 0 {
                ffi::munmap(self.ptr as *mut std::ffi::c_void, self.capacity);
            }
            if self.fd >= 0 {
                ffi::close(self.fd);
            }
        }
    }
}

pub struct WaylandClient {
    stream: UnixStream,
    rx_buf: Vec<u8>,
    next_id: u32,
    registry_id: u32,
    compositor_id: u32,
    compositor_version: u32,
    shm_id: u32,
    layer_shell_id: u32,
    empty_region_id: u32,
    pub outputs: Vec<WaylandOutput>,
    shm_pool: Option<ShmPool>,
    surface_id: u32,
    layer_surface_id: u32,
    bound_output_id: u32,
    applied_position: Option<Position>,
    applied_margin: i32,
    applied_size: (u32, u32),
    pub configured: bool,
    pub needs_redraw: bool,
    active_buffers: Vec<u32>,
}

impl WaylandClient {
    pub fn connect() -> io::Result<Self> {
        let sock_path = wayland_socket_path()
            .ok_or_else(|| io::Error::new(ErrorKind::NotFound, "WAYLAND_DISPLAY not found"))?;
        let stream = UnixStream::connect(sock_path)?;

        let mut client = Self {
            stream,
            rx_buf: Vec::with_capacity(4096),
            next_id: 2,
            registry_id: 0,
            compositor_id: 0,
            compositor_version: 1,
            shm_id: 0,
            layer_shell_id: 0,
            empty_region_id: 0,
            outputs: Vec::new(),
            shm_pool: None,
            surface_id: 0,
            layer_surface_id: 0,
            bound_output_id: 0,
            applied_position: None,
            applied_margin: -1,
            applied_size: (0, 0),
            configured: false,
            needs_redraw: false,
            active_buffers: Vec::new(),
        };

        client.registry_id = client.alloc_id();
        client.send_msg(
            WL_DISPLAY_ID,
            WL_DISPLAY_GET_REGISTRY,
            &[client.registry_id],
        )?;

        // Roundtrip 1: discover globals and bind compositor, shm, layer_shell, and outputs
        client.roundtrip()?;

        if client.compositor_id == 0 || client.shm_id == 0 || client.layer_shell_id == 0 {
            return Err(io::Error::new(
                ErrorKind::Unsupported,
                "compositor missing wl_compositor, wl_shm, or zwlr_layer_shell_v1",
            ));
        }

        // Create empty wl_region for click-through input region
        client.empty_region_id = client.alloc_id();
        client.send_msg(
            client.compositor_id,
            WL_COMPOSITOR_CREATE_REGION,
            &[client.empty_region_id],
        )?;

        // Roundtrip 2: receive initial wl_output name/geometry/scale events
        client.roundtrip()?;

        client.stream.set_nonblocking(true)?;
        Ok(client)
    }

    pub fn fd(&self) -> RawFd {
        self.stream.as_raw_fd()
    }

    fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn send_msg(&mut self, obj_id: u32, opcode: u16, words: &[u32]) -> io::Result<()> {
        let size = ((words.len() + 2) * 4) as u32;
        let mut buf = Vec::with_capacity(size as usize);
        buf.extend_from_slice(&obj_id.to_ne_bytes());
        buf.extend_from_slice(&((size << 16) | u32::from(opcode)).to_ne_bytes());
        for &w in words {
            buf.extend_from_slice(&w.to_ne_bytes());
        }
        self.stream.write_all(&buf)
    }

    fn send_msg_with_fd(
        &mut self,
        obj_id: u32,
        opcode: u16,
        words: &[u32],
        fd: RawFd,
    ) -> io::Result<()> {
        let size = ((words.len() + 2) * 4) as u32;
        let mut buf = Vec::with_capacity(size as usize);
        buf.extend_from_slice(&obj_id.to_ne_bytes());
        buf.extend_from_slice(&((size << 16) | u32::from(opcode)).to_ne_bytes());
        for &w in words {
            buf.extend_from_slice(&w.to_ne_bytes());
        }

        let mut iov = ffi::iovec {
            iov_base: buf.as_mut_ptr() as *mut std::ffi::c_void,
            iov_len: buf.len(),
        };

        #[repr(C)]
        struct CmsgFd {
            hdr: ffi::cmsghdr,
            fd: RawFd,
        }

        let mut cmsg = CmsgFd {
            hdr: ffi::cmsghdr {
                cmsg_len: std::mem::size_of::<ffi::cmsghdr>() + std::mem::size_of::<RawFd>(),
                cmsg_level: ffi::SOL_SOCKET,
                cmsg_type: ffi::SCM_RIGHTS,
            },
            fd,
        };

        let msg = ffi::msghdr {
            msg_name: ptr::null_mut(),
            msg_namelen: 0,
            msg_iov: &mut iov,
            msg_iovlen: 1,
            msg_control: &mut cmsg as *mut CmsgFd as *mut std::ffi::c_void,
            msg_controllen: std::mem::size_of::<CmsgFd>(),
            msg_flags: 0,
        };

        let sent = unsafe { ffi::sendmsg(self.stream.as_raw_fd(), &msg, ffi::MSG_NOSIGNAL) };
        if sent < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn roundtrip(&mut self) -> io::Result<()> {
        let cb_id = self.alloc_id();
        self.send_msg(WL_DISPLAY_ID, WL_DISPLAY_SYNC, &[cb_id])?;
        let mut done = false;
        while !done {
            self.read_and_dispatch(Some(cb_id), &mut done)?;
        }
        Ok(())
    }

    pub fn dispatch_pending(&mut self) -> io::Result<()> {
        loop {
            let mut dummy = false;
            match self.read_and_dispatch(None, &mut dummy) {
                Ok(0) => break,
                Ok(_) => continue,
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    fn read_and_dispatch(
        &mut self,
        wait_cb: Option<u32>,
        cb_done: &mut bool,
    ) -> io::Result<usize> {
        let mut tmp = [0u8; 4096];
        let n = self.stream.read(&mut tmp)?;
        if n == 0 {
            return Err(io::Error::new(
                ErrorKind::UnexpectedEof,
                "Wayland socket closed",
            ));
        }
        self.rx_buf.extend_from_slice(&tmp[..n]);

        let mut offset = 0usize;
        while self.rx_buf.len() - offset >= 8 {
            let sender_id = u32::from_ne_bytes(self.rx_buf[offset..offset + 4].try_into().unwrap());
            let word1 = u32::from_ne_bytes(self.rx_buf[offset + 4..offset + 8].try_into().unwrap());
            let msg_len = (word1 >> 16) as usize;
            let opcode = (word1 & 0xFFFF) as u16;

            if msg_len < 8 || self.rx_buf.len() - offset < msg_len {
                break;
            }

            let payload = self.rx_buf[offset + 8..offset + msg_len].to_vec();
            offset += msg_len;

            if Some(sender_id) == wait_cb && opcode == WL_CALLBACK_DONE {
                *cb_done = true;
            } else {
                self.handle_event(sender_id, opcode, &payload)?;
            }
        }

        if offset > 0 {
            self.rx_buf.drain(..offset);
        }
        Ok(n)
    }

    fn handle_event(&mut self, sender_id: u32, opcode: u16, payload: &[u8]) -> io::Result<()> {
        if sender_id == WL_DISPLAY_ID && opcode == WL_DISPLAY_ERROR {
            if payload.len() >= 8 {
                let obj = u32::from_ne_bytes(payload[0..4].try_into().unwrap());
                let code = u32::from_ne_bytes(payload[4..8].try_into().unwrap());
                let (msg, _) = decode_wl_string(&payload[8..]).unwrap_or_default();
                return Err(io::Error::other(format!(
                    "Wayland protocol error on object {obj} (code {code}): {msg}"
                )));
            }
        } else if sender_id == self.registry_id {
            if opcode == WL_REGISTRY_GLOBAL && payload.len() >= 8 {
                let name = u32::from_ne_bytes(payload[0..4].try_into().unwrap());
                if let Some((iface, consumed)) = decode_wl_string(&payload[4..]) {
                    let ver_off = 4 + consumed;
                    if payload.len() >= ver_off + 4 {
                        let version =
                            u32::from_ne_bytes(payload[ver_off..ver_off + 4].try_into().unwrap());
                        self.on_registry_global(name, &iface, version)?;
                    }
                }
            } else if opcode == WL_REGISTRY_GLOBAL_REMOVE && payload.len() >= 4 {
                let name = u32::from_ne_bytes(payload[0..4].try_into().unwrap());
                self.outputs.retain(|o| o.global_name != name);
            }
        } else if sender_id == self.layer_surface_id && self.layer_surface_id != 0 {
            if opcode == ZWLR_LAYER_SURFACE_CONFIGURE && payload.len() >= 12 {
                let serial = u32::from_ne_bytes(payload[0..4].try_into().unwrap());
                self.send_msg(
                    self.layer_surface_id,
                    ZWLR_LAYER_SURFACE_ACK_CONFIGURE,
                    &[serial],
                )?;
                self.configured = true;
                self.needs_redraw = true;
            } else if opcode == ZWLR_LAYER_SURFACE_CLOSED {
                self.destroy_surface()?;
            }
        } else if let Some(pos) = self.active_buffers.iter().position(|&b| b == sender_id) {
            if opcode == WL_BUFFER_RELEASE {
                let buf_id = self.active_buffers.swap_remove(pos);
                let _ = self.send_msg(buf_id, WL_BUFFER_DESTROY, &[]);
            }
        } else if let Some(out) = self.outputs.iter_mut().find(|o| o.obj_id == sender_id) {
            match opcode {
                WL_OUTPUT_GEOMETRY if payload.len() >= 20 => {
                    out.x = i32::from_ne_bytes(payload[0..4].try_into().unwrap());
                    out.y = i32::from_ne_bytes(payload[4..8].try_into().unwrap());
                    if let Some((make, c1)) = decode_wl_string(&payload[20..]) {
                        out.make = make;
                        let m_off = 20 + c1;
                        if payload.len() > m_off {
                            if let Some((model, _)) = decode_wl_string(&payload[m_off..]) {
                                out.model = model;
                            }
                        }
                    }
                }
                WL_OUTPUT_SCALE if payload.len() >= 4 => {
                    let s = i32::from_ne_bytes(payload[0..4].try_into().unwrap());
                    out.scale = s.max(1);
                }
                WL_OUTPUT_NAME => {
                    if let Some((name, _)) = decode_wl_string(payload) {
                        out.name = name;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn on_registry_global(&mut self, name: u32, iface: &str, version: u32) -> io::Result<()> {
        match iface {
            "wl_compositor" if self.compositor_id == 0 => {
                let v = version.min(4);
                let id = self.alloc_id();
                self.compositor_id = id;
                self.compositor_version = v;
                self.bind_global(name, iface, v, id)?;
            }
            "wl_shm" if self.shm_id == 0 => {
                let id = self.alloc_id();
                self.shm_id = id;
                self.bind_global(name, iface, 1, id)?;
            }
            "zwlr_layer_shell_v1" if self.layer_shell_id == 0 => {
                let v = version.min(4);
                let id = self.alloc_id();
                self.layer_shell_id = id;
                self.bind_global(name, iface, v, id)?;
            }
            "wl_output" => {
                let v = version.min(4);
                let id = self.alloc_id();
                self.outputs.push(WaylandOutput {
                    global_name: name,
                    obj_id: id,
                    scale: 1,
                    ..Default::default()
                });
                self.bind_global(name, iface, v, id)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn bind_global(&mut self, name: u32, iface: &str, version: u32, new_id: u32) -> io::Result<()> {
        let mut words = Vec::with_capacity(8);
        words.push(name);
        encode_wl_string(iface, &mut words);
        words.push(version);
        words.push(new_id);
        self.send_msg(self.registry_id, WL_REGISTRY_BIND, &words)
    }

    pub fn resolve_output_id(&self, target: Option<&MonitorTarget>) -> u32 {
        let Some(t) = target else {
            return 0;
        };

        // 1. Exact connector name match (wl_output v4 `name`, e.g. "eDP-1", "DP-1")
        if !t.name.is_empty() {
            if let Some(o) = self.outputs.iter().find(|o| o.name == t.name) {
                return o.obj_id;
            }
        }

        // 2. Exact layout origin (x, y) match
        if let Some(o) = self.outputs.iter().find(|o| o.x == t.x && o.y == t.y) {
            return o.obj_id;
        }

        // 3. Non-empty make + model match
        if !t.make.is_empty() || !t.model.is_empty() {
            if let Some(o) = self.outputs.iter().find(|o| {
                (t.make.is_empty() || o.make == t.make)
                    && (t.model.is_empty() || o.model == t.model)
            }) {
                return o.obj_id;
            }
        }

        0
    }

    pub fn ensure_surface(
        &mut self,
        output_id: u32,
        position: Position,
        margin: i32,
        width: u32,
        height: u32,
    ) -> io::Result<()> {
        let need_recreate = self.layer_surface_id == 0 || self.bound_output_id != output_id;

        if need_recreate {
            self.destroy_surface()?;

            let surface_id = self.alloc_id();
            let layer_surface_id = self.alloc_id();
            self.surface_id = surface_id;
            self.layer_surface_id = layer_surface_id;
            self.bound_output_id = output_id;
            self.configured = false;

            self.send_msg(
                self.compositor_id,
                WL_COMPOSITOR_CREATE_SURFACE,
                &[surface_id],
            )?;
            self.send_msg(
                surface_id,
                WL_SURFACE_SET_INPUT_REGION,
                &[self.empty_region_id],
            )?;

            let mut words = Vec::with_capacity(8);
            words.push(layer_surface_id);
            words.push(surface_id);
            words.push(output_id);
            words.push(ZWLR_LAYER_SHELL_LAYER_OVERLAY);
            encode_wl_string("workspace-indicator", &mut words);
            self.send_msg(
                self.layer_shell_id,
                ZWLR_LAYER_SHELL_GET_LAYER_SURFACE,
                &words,
            )?;

            self.send_msg(
                layer_surface_id,
                ZWLR_LAYER_SURFACE_SET_EXCLUSIVE_ZONE,
                &[0],
            )?;
            self.send_msg(
                layer_surface_id,
                ZWLR_LAYER_SURFACE_SET_KEYBOARD_INTERACTIVITY,
                &[0],
            )?;

            self.apply_geometry(position, margin, width, height, true)?;
            self.send_msg(surface_id, WL_SURFACE_COMMIT, &[])?;
        } else if self.applied_position != Some(position)
            || self.applied_margin != margin
            || self.applied_size != (width, height)
        {
            self.apply_geometry(position, margin, width, height, false)?;
        }

        Ok(())
    }

    fn apply_geometry(
        &mut self,
        position: Position,
        margin: i32,
        width: u32,
        height: u32,
        force: bool,
    ) -> io::Result<()> {
        if force || self.applied_position != Some(position) || self.applied_margin != margin {
            let (anchor, margins) = match position {
                Position::Top => (ANCHOR_TOP, [margin, 0, 0, 0]),
                Position::Right => (ANCHOR_RIGHT, [0, margin, 0, 0]),
                Position::Bottom => (ANCHOR_BOTTOM, [0, 0, margin, 0]),
                Position::Left => (ANCHOR_LEFT, [0, 0, 0, margin]),
            };
            self.send_msg(
                self.layer_surface_id,
                ZWLR_LAYER_SURFACE_SET_ANCHOR,
                &[anchor],
            )?;
            self.send_msg(
                self.layer_surface_id,
                ZWLR_LAYER_SURFACE_SET_MARGIN,
                &[
                    margins[0] as u32,
                    margins[1] as u32,
                    margins[2] as u32,
                    margins[3] as u32,
                ],
            )?;
            self.applied_position = Some(position);
            self.applied_margin = margin;
        }

        if force || self.applied_size != (width, height) {
            self.send_msg(
                self.layer_surface_id,
                ZWLR_LAYER_SURFACE_SET_SIZE,
                &[width, height],
            )?;
            self.applied_size = (width, height);
        }
        Ok(())
    }

    pub fn destroy_surface(&mut self) -> io::Result<()> {
        if self.layer_surface_id != 0 {
            let _ = self.send_msg(self.layer_surface_id, ZWLR_LAYER_SURFACE_DESTROY, &[]);
            self.layer_surface_id = 0;
        }
        if self.surface_id != 0 {
            let _ = self.send_msg(self.surface_id, WL_SURFACE_DESTROY, &[]);
            self.surface_id = 0;
        }
        self.configured = false;
        self.applied_position = None;
        self.applied_size = (0, 0);
        Ok(())
    }

    pub fn present_frame<F>(&mut self, width: u32, height: u32, render_fn: F) -> io::Result<()>
    where
        F: FnOnce(&mut [u32]),
    {
        if !self.configured || self.surface_id == 0 || width == 0 || height == 0 {
            return Ok(());
        }

        let stride = width * 4;
        let frame_bytes = (stride * height) as usize;
        let needed_capacity = frame_bytes * 2; // double-buffered pool

        self.ensure_shm_pool(needed_capacity)?;

        let (pool_id, slot_offset, buf_slice) = {
            let pool = self.shm_pool.as_mut().unwrap();
            let slot = pool.slot_idx & 1;
            pool.slot_idx = pool.slot_idx.wrapping_add(1);
            let byte_offset = slot * frame_bytes;
            let pixels = unsafe {
                slice::from_raw_parts_mut(
                    pool.ptr.add(byte_offset) as *mut u32,
                    (width * height) as usize,
                )
            };
            (pool.pool_id, byte_offset as i32, pixels)
        };

        render_fn(buf_slice);

        let buffer_id = self.alloc_id();
        self.send_msg(
            pool_id,
            WL_SHM_POOL_CREATE_BUFFER,
            &[
                buffer_id,
                slot_offset as u32,
                width,
                height,
                stride,
                WL_SHM_FORMAT_ARGB8888,
            ],
        )?;
        self.active_buffers.push(buffer_id);

        self.send_msg(self.surface_id, WL_SURFACE_ATTACH, &[buffer_id, 0, 0])?;
        if self.compositor_version >= 4 {
            self.send_msg(
                self.surface_id,
                WL_SURFACE_DAMAGE_BUFFER,
                &[0, 0, width, height],
            )?;
        } else {
            self.send_msg(self.surface_id, WL_SURFACE_DAMAGE, &[0, 0, width, height])?;
        }
        self.send_msg(self.surface_id, WL_SURFACE_COMMIT, &[])?;
        Ok(())
    }

    fn ensure_shm_pool(&mut self, min_capacity: usize) -> io::Result<()> {
        let page_aligned = (min_capacity.max(65536) + 4095) & !4095;

        if let Some(pool) = &mut self.shm_pool {
            if pool.capacity >= page_aligned {
                return Ok(());
            }
            // Resize existing memfd and wl_shm_pool
            if unsafe { ffi::ftruncate(pool.fd, page_aligned as ffi::off_t) } < 0 {
                return Err(io::Error::last_os_error());
            }
            let new_ptr = unsafe {
                ffi::mmap(
                    ptr::null_mut(),
                    page_aligned,
                    ffi::PROT_READ | ffi::PROT_WRITE,
                    ffi::MAP_SHARED,
                    pool.fd,
                    0,
                )
            };
            if new_ptr == ffi::MAP_FAILED {
                return Err(io::Error::last_os_error());
            }
            unsafe {
                ffi::munmap(pool.ptr as *mut std::ffi::c_void, pool.capacity);
            }
            pool.ptr = new_ptr as *mut u8;
            pool.capacity = page_aligned;
            let pool_id = pool.pool_id;
            self.send_msg(pool_id, WL_SHM_POOL_RESIZE, &[page_aligned as u32])?;
            return Ok(());
        }

        let name = CString::new("workspace-indicator-shm").unwrap();
        let fd = unsafe { ffi::memfd_create(name.as_ptr(), ffi::MFD_CLOEXEC) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        if unsafe { ffi::ftruncate(fd, page_aligned as ffi::off_t) } < 0 {
            unsafe {
                ffi::close(fd);
            }
            return Err(io::Error::last_os_error());
        }
        let ptr = unsafe {
            ffi::mmap(
                ptr::null_mut(),
                page_aligned,
                ffi::PROT_READ | ffi::PROT_WRITE,
                ffi::MAP_SHARED,
                fd,
                0,
            )
        };
        if ptr == ffi::MAP_FAILED {
            unsafe {
                ffi::close(fd);
            }
            return Err(io::Error::last_os_error());
        }

        let pool_id = self.alloc_id();
        self.send_msg_with_fd(
            self.shm_id,
            WL_SHM_CREATE_POOL,
            &[pool_id, page_aligned as u32],
            fd,
        )?;

        self.shm_pool = Some(ShmPool {
            fd,
            pool_id,
            ptr: ptr as *mut u8,
            capacity: page_aligned,
            slot_idx: 0,
        });
        Ok(())
    }
}

fn wayland_socket_path() -> Option<PathBuf> {
    let display = env::var("WAYLAND_DISPLAY").ok()?;
    if display.is_empty() {
        return None;
    }
    let p = PathBuf::from(&display);
    if p.is_absolute() && p.exists() {
        return Some(p);
    }
    let xdg = env::var("XDG_RUNTIME_DIR").ok()?;
    let candidate = PathBuf::from(xdg).join(display);
    if candidate.exists() {
        Some(candidate)
    } else {
        None
    }
}

fn encode_wl_string(s: &str, out_words: &mut Vec<u32>) {
    let bytes = s.as_bytes();
    let len_with_nul = bytes.len() + 1;
    out_words.push(len_with_nul as u32);

    let padded = (len_with_nul + 3) & !3;
    let mut buf = vec![0u8; padded];
    buf[..bytes.len()].copy_from_slice(bytes);
    for chunk in buf.chunks_exact(4) {
        out_words.push(u32::from_ne_bytes(chunk.try_into().unwrap()));
    }
}

fn decode_wl_string(buf: &[u8]) -> Option<(String, usize)> {
    if buf.len() < 4 {
        return None;
    }
    let len = u32::from_ne_bytes(buf[0..4].try_into().unwrap()) as usize;
    if len == 0 {
        return Some((String::new(), 4));
    }
    let padded = (len + 3) & !3;
    if buf.len() < 4 + padded {
        return None;
    }
    let str_bytes = &buf[4..4 + len.saturating_sub(1)];
    let s = String::from_utf8_lossy(str_bytes).into_owned();
    Some((s, 4 + padded))
}
