import "./styles.css"

import { render } from "solid-js/web"
import { App } from "./App"
import { readBootstrap } from "./bootstrap"

const root = document.getElementById("keith-app")
if (!(root instanceof HTMLElement)) throw new Error("Keith application root is unavailable")

render(() => <App bootstrap={readBootstrap(root)} />, root)
