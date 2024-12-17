use yew::prelude::*;

#[function_component(App)]
pub fn app() -> Html {
    html! {
        <div class="overlay">
            <div class="status-indicator"></div>
            <span class="status-text">{"Listening..."}</span>
        </div>
    }
}
