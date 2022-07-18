//! Process management syscalls

use core::panic;

use riscv::addr::page;

use crate::config::{MAX_SYSCALL_NUM, PAGE_SIZE};
use crate::mm::{VirtAddr, MapType, 
    MapPermission, MapArea, PageTable, StepByOne, translated_refmut};
use crate::task::{exit_current_and_run_next, suspend_current_and_run_next, 
    TaskStatus, current_user_token, current_task_info, set_current_task_running_time, 
    current_task_insert_mm, current_task_unmap_area};
use crate::timer::get_time_us;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct TaskInfo {
    pub status: TaskStatus,
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
    pub time: usize,
}

pub fn sys_exit(exit_code: i32) -> ! {
    info!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}

// YOUR JOB: 引入虚地址后重写 sys_get_time
// trap后内核无法通过指针访问user数据, 考虑类似sys_write的改进
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    let _us = get_time_us();
    set_current_task_running_time(_us / 1_000);
    let true_ts = translated_refmut(current_user_token(), _ts);
    // info!("get & mut ts");
    (*true_ts).sec = _us /1_000_000;
    (*true_ts).usec = _us % 1_000_000;
    0
}

// CLUE: 从 ch4 开始不再对调度算法进行测试~
pub fn sys_set_priority(_prio: isize) -> isize {
    -1
}

// YOUR JOB: 扩展内核以实现 sys_mmap 和 sys_munmap
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    if (_port & !0x7 != 0) || (_port & 0x7 == 0) {
        return -1;
    }
    if _start % PAGE_SIZE != 0 {
        return -1;
    }
    // get permission
    let mut mm_perm = MapPermission::U;
    if (_port & 0x01) != 0x00 {
        mm_perm |= MapPermission::R;
    }
    if (_port & 0x02) != 0x00 {
        mm_perm |= MapPermission::W;
    }
    if (_port & 0x04) != 0x00 {
        mm_perm |= MapPermission::X;
    }
    // virt range that should allocate
    let start_va = VirtAddr::from(_start);
    let end_va = VirtAddr::from(_start + _len);
    // check if area is allocated
    let page_table = PageTable::from_token(current_user_token());
    let mut cur_vpn = start_va.floor();
    let end_vpn = end_va.ceil(); 
    while cur_vpn.0 < end_vpn.0 {
        if let Some(entry) = page_table.translate(cur_vpn) {
            if entry.is_valid() {
                return -1;
            }
        }
        cur_vpn.step();
    }
    // insert map_area to user's memset
    current_task_insert_mm(start_va, end_va, mm_perm);
    0
}

pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    // virt range that should unallocate
    let start_va = VirtAddr::from(_start);
    let end_va = VirtAddr::from(_start + _len);
    // check if an area is unallocated
    let page_table = PageTable::from_token(current_user_token());
    let mut cur_vpn = start_va.floor();
    let end_vpn = end_va.ceil();
    while cur_vpn.0 <= end_vpn.0 {
        match page_table.translate(cur_vpn) {
            None => { return -1; }
            Some(entry) => {
                if !entry.is_valid() {
                    return -1;
                }
            }
        }
        cur_vpn.step();
    }
    // get map_area of user
    current_task_unmap_area(start_va, end_va);
    0
}

// YOUR JOB: 引入虚地址后重写 sys_task_info
pub fn sys_task_info(ti: *mut TaskInfo) -> isize {
    // call task control block for info
    let (s, st, t) = current_task_info();
    let true_task_info = translated_refmut(current_user_token(), ti);
    (*true_task_info).status = s;
    (*true_task_info).syscall_times = st;
    (*true_task_info).time = t;
    0
}
