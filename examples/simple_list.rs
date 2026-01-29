use dioxus::{
    document::{Eval, eval},
    prelude::*,
};
use dioxus_dnd_kit::prelude::*;

fn main() {
    launch(App);
}

#[derive(PartialEq, Clone, Debug)]
struct Task {
    id: usize,
    name: String,
    height: f64,
}

#[component]
fn App() -> Element {
    let tasks = use_signal(|| {
        vec![
            Task {
                id: 1,
                name: "Buy Coca-Cola".into(),
                height: 50.0,
            },
            Task {
                id: 2,
                name: "Get It!...".into(),
                height: 120.0,
            },
            Task {
                id: 3,
                name: "...sell half coca cola?".into(),
                height: 80.0,
            },
        ]
    });

    rsx! {
        document::Script { src: "https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4" }

        DraggableView {
            class: "p-10 bg-gray-100 min-h-screen",
            key_gen: |task: Task| task.id.to_string(),
            render: move |task: Task| rsx! {
                div {
                    class: "max-w-64 bg-blue-500 text-white flex items-center justify-center rounded shadow-lg",
                    style: "height: {task.height}px;",
                    DraggableHandler { class: "cursor-move",
                        item: task.clone(),
                        "{task.name}"
                    }
                }
            },

            DndTailwindListener {}

            div { class: "p-24 flex justify-center",
                Droppable {
                    class: "bg-gray-100 p-8 rounded-xl w-76 grid gap-4 ",
                    items: tasks,
                    // Тут мы и проверим, как работает твоя живая сортировка
                }
            }
        }
    }
}

#[component]
fn DndTailwindListener() -> Element {
    let mut context = use_context::<DndContext<Task>>();
    let mut watcher = use_signal(|| Option::<Eval>::None);

    use_effect(move || {
        spawn(async move {
            let mut eval_item = eval(
                r#"
                let timer;
                const observer = new MutationObserver(() => {
                  // Проверяем, появился ли стайл-тег от tailwind
                  if (document.querySelector('style')) { 
                    // Tailwind v4 обычно инжектит стили сразу.
                    // Твой колбэк на пересчет rect'ов здесь:
                    clearTimeout(timer);
                    timer = setTimeout(() => {
                      dioxus.send("updated");
                    }, 50);
                    console.log("Tailwind styles injected");
                  }
                });
                
                observer.observe(document.head, { childList: true, subtree: true });
                
                await dioxus.recv();
                observer.disconnect();
                "#,
            );
            if let Some(old_eval) = watcher() {
                let _ = old_eval.send("drop");
            }
            watcher.set(Some(eval_item));

            loop {
                if let Ok(_) = eval_item.recv::<String>().await {
                    context.recalculate_rects.with_mut(|r| *r += 1);
                } else {
                    break;
                }
            }
        });
    });

    use_drop(move || {
        if let Some(old_eval) = watcher() {
            let _ = old_eval.send("drop");
        }
    });

    rsx! {}
}
