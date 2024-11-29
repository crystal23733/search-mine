/******/ (() => { // webpackBootstrap
/******/ 	"use strict";
/******/ 	var __webpack_modules__ = ({

/***/ "./src/main/index.ts":
/*!***************************!*\
  !*** ./src/main/index.ts ***!
  \***************************/
/***/ (function(__unused_webpack_module, exports, __webpack_require__) {


var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", ({ value: true }));
const electron_1 = __webpack_require__(/*! electron */ "electron");
const path_1 = __importDefault(__webpack_require__(/*! path */ "path"));
// 메인 윈도우 생성 함수
const createWindow = () => {
    // 브라우저 윈도우 인스턴스 생성
    const mainWindow = new electron_1.BrowserWindow({
        width: 800,
        height: 600,
        webPreferences: {
            nodeIntegration: true, // react에서 Node.js API를 사용할 수 있게 해줌
            contextIsolation: false, // true면 메인 프로세스와 렌더러 프로세스가 격리됨, false면 직접 통신 가능(개발할 때는 편의상 false)
        },
    });
    // HTML파일을 창에 로드
    // __dirname: 현재 파일의 디렉토리 경로
    mainWindow.loadFile(path_1.default.join(__dirname, '../../src/renderer/index.html'));
    // 개발자 도구 창 열기 (console.log)
    mainWindow.webContents.openDevTools();
};
// app.whenReady(): Electron 앱이 초기화를 마치면 실행
electron_1.app.whenReady().then(() => {
    createWindow(); // 첫 창 생성
    // macOS에서만 해당되는 이벤트 핸들러
    // dock 아이콘 클릭시 창이 없으면 새로 생성
    electron_1.app.on('activate', () => {
        if (electron_1.BrowserWindow.getAllWindows().length === 0) {
            createWindow();
        }
    });
});
// 모든 창이 닫혔을 때의 이벤트 핸들러
electron_1.app.on('window-all-closed', () => {
    // macOS가 아니면 앱 종료
    // macOS는 창을 다 닫아도 dock에 아이콘이 남아있는게 기본 동작
    if (process.platform !== 'darwin') {
        electron_1.app.quit();
    }
});


/***/ }),

/***/ "electron":
/*!***************************!*\
  !*** external "electron" ***!
  \***************************/
/***/ ((module) => {

module.exports = require("electron");

/***/ }),

/***/ "path":
/*!***********************!*\
  !*** external "path" ***!
  \***********************/
/***/ ((module) => {

module.exports = require("path");

/***/ })

/******/ 	});
/************************************************************************/
/******/ 	// The module cache
/******/ 	var __webpack_module_cache__ = {};
/******/ 	
/******/ 	// The require function
/******/ 	function __webpack_require__(moduleId) {
/******/ 		// Check if module is in cache
/******/ 		var cachedModule = __webpack_module_cache__[moduleId];
/******/ 		if (cachedModule !== undefined) {
/******/ 			return cachedModule.exports;
/******/ 		}
/******/ 		// Create a new module (and put it into the cache)
/******/ 		var module = __webpack_module_cache__[moduleId] = {
/******/ 			// no module.id needed
/******/ 			// no module.loaded needed
/******/ 			exports: {}
/******/ 		};
/******/ 	
/******/ 		// Execute the module function
/******/ 		__webpack_modules__[moduleId].call(module.exports, module, module.exports, __webpack_require__);
/******/ 	
/******/ 		// Return the exports of the module
/******/ 		return module.exports;
/******/ 	}
/******/ 	
/************************************************************************/
/******/ 	
/******/ 	// startup
/******/ 	// Load entry module and return exports
/******/ 	// This entry module is referenced by other modules so it can't be inlined
/******/ 	var __webpack_exports__ = __webpack_require__("./src/main/index.ts");
/******/ 	
/******/ })()
;
//# sourceMappingURL=index.js.map