use super::{Task, TaskId};
use alloc::{collections::BTreeMap, sync::Arc, task::Wake};
use crossbeam_queue::ArrayQueue;
use core::task::{Context, Poll, Waker};

extern "C" {
    fn _glue_irq_save() -> u8;
    fn _glue_irq_restore(flags: u8);
    fn _glue_yield();
}

#[derive(Debug)]
pub struct Executor {
    tasks: BTreeMap<TaskId, Task>,
    task_queue: Arc<ArrayQueue<TaskId>>,
    waker_cache: BTreeMap<TaskId, Waker>,
}

impl Default for Executor {
    fn default() -> Executor {
        Executor::new()
    }
}

// Struct representing the executor which manages tasks and their execution.
impl Executor {
    // Constructor for Executor. Initializes the tasks, task_queue and waker_cache.
    pub fn new() -> Self {
        Executor {
            tasks: BTreeMap::new(),
            task_queue: Arc::new(ArrayQueue::new(100)),
            waker_cache: BTreeMap::new(),
        }
    }

    // Spawns a new task. 
    // Inserts the task into tasks map and pushes the task_id into the task_queue.
    pub fn spawn(&mut self, task: Task) {
        let task_id = task.id;
        if self.tasks.insert(task.id, task).is_some() {
            panic!("task with same ID already in tasks");
        }
        self.task_queue.push(task_id).expect("queue full");
    }

    // Runs the executor, which continuously executes 
    // tasks if they're ready, and sleeps if idle.
    pub fn run(&mut self, exit_when_empty: bool) {
        loop {
            self.run_ready_tasks();
            self.sleep_if_idle();
            if exit_when_empty && self.tasks.is_empty() {
                break;
            }
        }
    }

    // Runs tasks that are ready. 
    // If a task is finished, it's removed from the tasks 
    // and its waker is removed from the waker_cache.
    fn run_ready_tasks(&mut self) {
        // destructure `self` to avoid borrow checker errors
        let Self {
            tasks,
            task_queue,
            waker_cache,
        } = self;

        while let Some(task_id) = task_queue.pop() {
            let task = match tasks.get_mut(&task_id) {
                Some(task) => task,
                None => continue, // task no longer exists
            };
            let waker = waker_cache
                .entry(task_id)
                .or_insert_with(|| TaskWaker::make_waker(task_id, task_queue.clone()));
            let mut context = Context::from_waker(waker);
            match task.poll(&mut context) {
                Poll::Ready(()) => {
                    // task done -> remove it and its cached waker
                    tasks.remove(&task_id);
                    waker_cache.remove(&task_id);
                }
                Poll::Pending => {}
            }
        }
    }

    // Yields the executor thread if no tasks are in the queue. 
    // Otherwise, enables the interrupts.
    fn sleep_if_idle(&self) {
        unsafe {
            let iflag = _glue_irq_save();
            if self.task_queue.is_empty() {
                _glue_yield();
            }
            _glue_irq_restore(iflag);
        }
    }


}

// Struct representing a TaskWaker, which includes a task_id and a reference to the task_queue.
struct TaskWaker {
    task_id: TaskId,
    task_queue: Arc<ArrayQueue<TaskId>>,
}

// Implementation of TaskWaker.
impl TaskWaker {
    // Constructor for TaskWaker. 
    // Initializes with a given task_id and a reference to the task_queue.
    fn make_waker(task_id: TaskId, task_queue: Arc<ArrayQueue<TaskId>>) -> Waker {
        Waker::from(Arc::new(TaskWaker {
            task_id,
            task_queue,
        }))
    }

    // Pushes task_id into the task_queue to wake up the task.
    fn wake_task(&self) {
        self.task_queue.push(self.task_id).expect("task_queue full");
    }
}

// Implementation of Wake for TaskWaker.
// Defines how a TaskWaker can wake up a task.
impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_task();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_task();
    }
}
