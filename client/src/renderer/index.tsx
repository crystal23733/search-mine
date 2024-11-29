import React from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import './styles/globals.css';

console.log("React 앱이 시작되었습니다!");

const container = document.getElementById('root');
const root = createRoot(container!);

root.render(<App />); 