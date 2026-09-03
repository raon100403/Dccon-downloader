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
- ✅ Linux AppImage 지원

## 주의사항

 이 프로그램은 디시인사이드의 공식 프로그램이 아닙니다.
- 서비스 이용약관을 준수하여 사용하세요.
- 짧은 시간에 많은 디시콘을 반복적으로 다운로드하거나, 여러 환경에서 동시에 사용하는 행위는 서버에 부담을 줄 수 있으며 IP 제한 또는 이용 제한의 원인이 될 수 있고, 본인은 책임을 지지 않습니다.
- 제작자는 디시인사이드의 IP 차단, 이용 제한, 계정 제한, 서비스 정책 변경 또는 다운로드 중 발생하는 문제에 대해 보장하거나 책임지지 않습니다.
- 서버 오류, 요청 제한(예: HTTP 429), 네트워크 장애가 발생하면 반복해서 요청하지 말고 사용을 중단하세요.
- 사용자는 본인의 판단과 책임하에 이 프로그램을 사용해야 합니다.

프로그램을 사용함으로써 발생하는 서비스 이용 제한, 데이터 손실, 저작권 문제 및 기타 손해에 대한 책임은 사용자에게 있습니다.


## 라이센스

MIT
## 라이센스

MIT
