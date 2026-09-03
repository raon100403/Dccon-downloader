import { useState } from 'react'
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
  filename: string
  status: string
}

function App() {
  const [input, setInput] = useState('')
  const [packageIdx, setPackageIdx] = useState('')
  const [dcconInfo, setDcconInfo] = useState<DcconPackageInfo | null>(null)
  const [savePath, setSavePath] = useState('')
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)
  const [downloading, setDownloading] = useState(false)
  const [progress, setProgress] = useState<DownloadProgress | null>(null)
  const [downloadComplete, setDownloadComplete] = useState(false)

  const handleParse = async () => {
    if (!input.trim()) {
      setError('입력값이 비어있습니다')
      return
    }

    setLoading(true)
    setError('')
    setPackageIdx('')
    setDcconInfo(null)
    setDownloadComplete(false)

    try {
      const idx = await invoke<string>('parse_dccon_url', { input })
      setPackageIdx(idx)

      // Automatically fetch dccon info after parsing
      await fetchDcconInfo(idx)
    } catch (err) {
      setError(`파싱 실패: ${err}`)
    } finally {
      setLoading(false)
    }
  }

  const fetchDcconInfo = async (idx: string) => {
    setLoading(true)
    setError('')

    try {
      const info = await invoke<DcconPackageInfo>('fetch_dccon_info', {
        packageIdx: idx,
      })
      setDcconInfo(info)
    } catch (err) {
      setError(`디시콘 정보 조회 실패: ${err}`)
    } finally {
      setLoading(false)
    }
  }

  const handleSelectPath = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: '디시콘 저장 경로 선택',
      })

      if (selected) {
        setSavePath(selected as string)
      }
    } catch (err) {
      setError(`경로 선택 실패: ${err}`)
    }
  }

  const handleDownload = async () => {
    if (!dcconInfo || !savePath) {
      setError('디시콘 정보와 저장 경로를 확인하세요')
      return
    }

    setDownloading(true)
    setError('')
    setProgress(null)
    setDownloadComplete(false)

    try {
      const onProgress = new Channel<DownloadProgress>()
      onProgress.onmessage = (progressData) => {
        setProgress(progressData)
      }

      await invoke<string>('download_dccon', {
        packageIdx: dcconInfo.package_idx,
        savePath: savePath,
        progress: onProgress,
      })

      setDownloadComplete(true)
      setError('')
    } catch (err) {
      setError(`다운로드 실패: ${err}`)
    } finally {
      setDownloading(false)
    }
  }

  const getProgressPercentage = () => {
    if (!progress || progress.total === 0) return 0
    return Math.round((progress.current / progress.total) * 100)
  }

  return (
    <div className="container">
      <h1>디시콘 다운로더</h1>
      <p>디시콘 URL 또는 번호를 입력하세요</p>

      <div className="input-group">
        <input
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder="예: 12345 또는 https://dccon.dcinside.com/detail/12345"
          onKeyPress={(e) => e.key === 'Enter' && handleParse()}
          disabled={loading || downloading}
        />
        <button onClick={handleParse} disabled={loading || downloading}>
          {loading ? '처리중...' : '조회하기'}
        </button>
      </div>

      {error && <div className="result error">{error}</div>}

      {packageIdx && !dcconInfo && !error && (
        <div className="result success">
          package_idx = {packageIdx} (정보 조회 중...)
        </div>
      )}

      {downloadComplete && (
        <div className="result success">
          ✅ 다운로드가 완료되었습니다! ({progress?.total}개 파일)
        </div>
      )}

      {dcconInfo && (
        <div className="dccon-info">
          <h2>{dcconInfo.title}</h2>
          <div className="info-details">
            <p>
              <strong>Package ID:</strong> {dcconInfo.package_idx}
            </p>
            <p>
              <strong>이미지 개수:</strong> {dcconInfo.image_count}개
            </p>
          </div>

          <div className="save-path-section">
            <h3>저장 경로:</h3>
            <div className="path-selector">
              <input
                type="text"
                value={savePath}
                readOnly
                placeholder="경로를 선택하세요"
                className="path-input"
              />
              <button
                onClick={handleSelectPath}
                className="select-btn"
                disabled={downloading}
              >
                📁 선택
              </button>
            </div>
            {savePath && !downloading && (
              <button className="download-btn" onClick={handleDownload}>
                ⬇️ 다운로드
              </button>
            )}
            {downloading && progress && (
              <div className="progress-container">
                <div className="progress-bar-wrapper">
                  <div
                    className="progress-bar"
                    style={{ width: `${getProgressPercentage()}%` }}
                  ></div>
                </div>
                <div className="progress-text">
                  <p className="progress-status">{progress.status}</p>
                  <p className="progress-details">
                    {progress.filename && `현재: ${progress.filename}`}
                  </p>
                  <p className="progress-percentage">
                    {getProgressPercentage()}%
                  </p>
                </div>
              </div>
            )}
          </div>

          <div className="image-preview">
            <h3>이미지 목록:</h3>
            <div className="image-list">
              {dcconInfo.images.slice(0, 10).map((img, idx) => (
                <div key={idx} className="image-item">
                  {idx}.{img.ext}
                </div>
              ))}
              {dcconInfo.image_count > 10 && (
                <div className="image-item more">
                  +{dcconInfo.image_count - 10} more...
                </div>
              )}
            </div>
          </div>
        </div>
      )}

      <div className="examples">
        <h3>입력 예시:</h3>
        <ul>
          <li>12345</li>
          <li>https://dccon.dcinside.com/detail/12345</li>
          <li>https://dccon.dcinside.com/index/package_detail?no=12345</li>
        </ul>
      </div>
    </div>
  )
}

export default App
