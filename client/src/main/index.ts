import { app, BrowserWindow } from "electron"
import path from "path";

// 메인 윈도우 생성 함수
const createWindow = () => {
  // 브라우저 윈도우 인스턴스 생성
  const mainWindow = new BrowserWindow({
    width: 800,
    height: 600,
    webPreferences: {
      nodeIntegration: true, // react에서 Node.js API를 사용할 수 있게 해줌
      contextIsolation: false, // true면 메인 프로세스와 렌더러 프로세스가 격리됨, false면 직접 통신 가능(개발할 때는 편의상 false)
    },
  });

  // HTML파일을 창에 로드
  // __dirname: 현재 파일의 디렉토리 경로
  mainWindow.loadFile(path.join(__dirname, '../../src/renderer/index.html'));

  // 개발자 도구 창 열기 (console.log)
  mainWindow.webContents.openDevTools();
}

// app.whenReady(): Electron 앱이 초기화를 마치면 실행
app.whenReady().then(() => {
  createWindow(); // 첫 창 생성

  // macOS에서만 해당되는 이벤트 핸들러
  // dock 아이콘 클릭시 창이 없으면 새로 생성
  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
});

// 모든 창이 닫혔을 때의 이벤트 핸들러
app.on('window-all-closed', () => {
  // macOS가 아니면 앱 종료
  // macOS는 창을 다 닫아도 dock에 아이콘이 남아있는게 기본 동작
  if(process.platform !== 'darwin') {
    app.quit();
  }
});