# 디시콘 다운로더 (DCCon Downloader)

디시인사이드의 디시콘을 다운로드할 수 있는 데스크톱 애플리케이션입니다.

## 필수 의존성 설치

### Fedora
```bash
sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel librsvg2-devel
```

### Ubuntu/Debian
```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

### Arch Linux
```bash
sudo pacman -S webkit2gtk-4.1 base-devel curl wget file openssl appmenu-gtk-module gtk3 libappindicator-gtk3 librsvg
```

## 개발 환경 설정

1. 의존성 설치 (위 참조)
2. Node.js와 Rust 설치 확인
   ```bash
   npm --version
   cargo --version
   ```
3. 프로젝트 의존성 설치
   ```bash
   npm install
   ```

## 개발 모드 실행

```bash
npm run tauri:dev
```

## 빌드

```bash
npm run tauri:build
```

빌드된 AppImage는 `src-tauri/target/release/bundle/appimage/`에 생성됩니다.

## 기능

- ✅ URL 또는 package_idx 입력으로 디시콘 검색
- ✅ 디시콘 이미지 일괄 다운로드
- ✅ 다운로드 진행 상황 실시간 표시
- ✅ 기본 다운로드 경로 저장
- ✅ Linux AppImage 지원

## 기술 스택

- **Frontend**: React + TypeScript + Vite
- **Backend**: Rust + Tauri
- **HTTP Client**: reqwest
- **Build**: AppImage

## 라이센스

MIT
