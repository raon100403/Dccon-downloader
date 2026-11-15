import { useRef, useState, type FormEvent } from 'react'
import { invoke, Channel } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import './App.css'

interface DcconPackageInfo {
  title: string
  package_idx: string
  image_count: number
  images: Array<{ path: string; ext: string }>
}

interface DownloadProgress {
  current: number
  total: number
  downloaded: number
  skipped: number
  filename: string
  status: string
}

interface DownloadResult {
  cancelled: boolean
  total: number
  downloaded: number
  skipped: number
  save_path: string
}

function App() {
  const [input, setInput] = useState('')
  const [info, setInfo] = useState<DcconPackageInfo | null>(null)
  const [savePath, setSavePath] = useState('')
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)
  const [downloading, setDownloading] = useState(false)
  const [cancelling, setCancelling] = useState(false)
  const [selectingPath, setSelectingPath] = useState(false)
  const [progress, setProgress] = useState<DownloadProgress | null>(null)
  const [result, setResult] = useState<DownloadResult | null>(null)
  const busy = useRef(false)
  const downloadId = useRef<string | null>(null)
  const cancelPending = useRef(false)

  const handleLookup = async (event: FormEvent) => {
    event.preventDefault()
    if (busy.current) return
    if (!input.trim()) {
      setError('디시콘 페이지 주소나 번호를 입력하세요.')
      return
    }
    busy.current = true
    setLoading(true)
    setError('')
    setInfo(null)
    setProgress(null)
    setResult(null)
    try {
      const packageIdx = await invoke<string>('parse_dccon_url', { input })
      const packageInfo = await invoke<DcconPackageInfo>('fetch_dccon_info', { packageIdx })
      setInfo(packageInfo)
    } catch (err) {
      setError(String(err))
    } finally {
      busy.current = false
      setLoading(false)
    }
  }

  const handleSelectPath = async () => {
    if (selectingPath || busy.current) return
    setSelectingPath(true)
    try {
      const selected = await open({ directory: true, multiple: false, title: '디시콘 저장 위치 선택' })
      if (typeof selected === 'string') {
        setSavePath(selected)
        setError('')
        setResult(null)
        setProgress(null)
      }
    } catch (err) {
      setError(`저장 위치 선택 실패: ${String(err)}`)
    } finally {
      setSelectingPath(false)
    }
  }

  const handleDownload = async () => {
    if (busy.current || !info || !savePath) return
    busy.current = true
    const id = crypto.randomUUID()
    downloadId.current = id
    setDownloading(true)
    setCancelling(false)
    setError('')
    setProgress(null)
    setResult(null)
    try {
      const channel = new Channel<DownloadProgress>()
      channel.onmessage = (update) => {
        if (downloadId.current === id) setProgress(update)
      }
      const outcome = await invoke<DownloadResult>('download_dccon', {
        packageIdx: info.package_idx, savePath, downloadId: id, progress: channel,
      })
      setResult(outcome)
      // Use the final command result even if the last channel message arrives later.
      setProgress({ current: outcome.downloaded + outcome.skipped, total: outcome.total,
        downloaded: outcome.downloaded, skipped: outcome.skipped, filename: '',
        status: outcome.cancelled ? '다운로드 중지됨' : '다운로드 완료' })
    } catch (err) {
      setError(String(err))
    } finally {
      downloadId.current = null
      cancelPending.current = false
      busy.current = false
      setDownloading(false)
      setCancelling(false)
    }
  }

  const handleCancel = async () => {
    const id = downloadId.current
    if (!id || cancelPending.current || !progress) return
    cancelPending.current = true
    setCancelling(true)
    try {
      await invoke('cancel_download', { downloadId: id })
    } catch (err) {
      if (downloadId.current === id) {
        cancelPending.current = false
        setCancelling(false)
        setError(`중지 요청 실패: ${String(err)}`)
      }
    }
  }

  const percentage = progress && progress.total > 0
    ? Math.floor((progress.current / progress.total) * 100) : 0
  const formats = info ? [...new Set(info.images.map((image) => image.ext.toUpperCase()))].join(', ') : ''
  const resumable = result?.cancelled || (error && progress)
  const statusTitle = error ? '작업을 완료하지 못했습니다'
    : downloading ? (cancelling ? '다운로드를 중지하고 있습니다' : progress?.status ?? '다운로드 준비 중')
      : result?.cancelled ? '다운로드를 중지했습니다'
        : result ? '다운로드를 완료했습니다' : ''

  return (
    <main className="app-shell">
      <header className="app-header">
        <h1>디시콘 다운로더</h1>
      </header>

      <form className="lookup-section" onSubmit={handleLookup} aria-busy={loading}>
        <label htmlFor="dccon-input">디시콘 주소 또는 번호</label>
        <div className="field-row">
          <input id="dccon-input" type="text" value={input}
            onChange={(event) => setInput(event.target.value)}
            placeholder="https://dccon.dcinside.com/#12345"
            aria-describedby="input-hint" autoComplete="off" spellCheck={false}
            disabled={loading || downloading} />
          <button type="submit" className="button-primary lookup-button" disabled={loading || downloading}>
            {loading ? '조회 중' : '조회'}
          </button>
        </div>
        <p className="field-hint" id="input-hint">페이지 주소를 붙여 넣거나 디시콘 번호만 입력하세요.</p>
      </form>

      <section className="package-section" aria-label="조회 결과" aria-busy={loading}>
        {info ? (
          <>
            <h2>{info.title}</h2>
            <dl className="package-facts">
              <div><dt>번호</dt><dd>{info.package_idx}</dd></div>
              <div><dt>이미지</dt><dd>{info.image_count}개</dd></div>
              <div><dt>형식</dt><dd>{formats}</dd></div>
            </dl>
          </>
        ) : (
          <p className="empty-state">{loading ? '디시콘 정보를 불러오고 있습니다.' : '조회한 디시콘 정보가 여기에 표시됩니다.'}</p>
        )}
      </section>

      <section className="save-section" aria-labelledby="save-label">
        <label id="save-label" htmlFor="save-path">저장 위치</label>
        <div className="field-row">
          <input id="save-path" value={savePath} readOnly placeholder="저장할 폴더를 선택하세요"
            className="path-input" title={savePath || undefined} />
          <button type="button" onClick={handleSelectPath} disabled={loading || downloading || selectingPath}>
            {selectingPath ? '선택 중' : '폴더 선택'}
          </button>
        </div>
        <p className="field-hint">선택한 위치에 디시콘별 폴더를 만듭니다.</p>
      </section>

      <div className="action-row">
        <p className="action-hint">{!info ? '먼저 디시콘을 조회하세요.' : !savePath ? '저장 위치를 선택하세요.' : '이미지는 원본 형식으로 저장됩니다.'}</p>
        <div className="action-buttons">
          {downloading && <button type="button" onClick={handleCancel} disabled={cancelling || !progress}>
            {cancelling ? '중지 중' : '중지'}
          </button>}
          <button type="button" className="button-primary download-button" onClick={handleDownload}
            disabled={!info || !savePath || loading || downloading || selectingPath}>
            {downloading ? '다운로드 중' : resumable ? '이어서 다운로드' : result ? '파일 다시 확인' : '다운로드'}
          </button>
        </div>
      </div>

      {statusTitle && (
        <section className="download-status" aria-label="작업 상태">
          <div className="status-heading" role={error ? 'alert' : 'status'}>
            <p>{statusTitle}</p>
            {progress && <span className="status-count">{progress.current} / {progress.total}</span>}
          </div>
          {error && <p className="status-detail">{error}</p>}
          {progress && (
            <>
              <progress value={progress.current} max={progress.total || 1} aria-label="저장 완료된 이미지" />
              <div className="progress-details">
                <span>새로 저장 {progress.downloaded}개 · 기존 파일 {progress.skipped}개</span>
                <span>{percentage}%</span>
              </div>
            </>
          )}
          {result && !error && <p className="status-detail">{result.cancelled
            ? '완료된 파일은 보관됩니다. 같은 위치에 이어서 다운로드할 수 있습니다.'
            : result.save_path}</p>}
        </section>
      )}

    </main>
  )
}

export default App
