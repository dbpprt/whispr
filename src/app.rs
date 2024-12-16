use yew::prelude::*;

#[function_component(App)]
pub fn app() -> Html {
    html! {
        <div>
            <h1>{"Whispr"}</h1>
            <p>{"Press right Option key to speak"}</p>
        </div>
    }
}
