import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const backend = vi.hoisted(() => ({
  invoke: vi.fn(),
  open: vi.fn(),
  channels: [] as Array<{ onmessage: (value: unknown) => void }>,
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke: backend.invoke,
  Channel: class {
    onmessage = (_value: unknown) => {}
    constructor() { backend.channels.push(this) }
  },
}))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: backend.open }))

import App from './App'

const packageInfo = {
  title: '테스트 디시콘', package_idx: '123', image_count: 2,
  images: [{ path: 'first', ext: 'gif' }, { path: 'second', ext: 'png' }],
}
const completed = { cancelled: false, total: 2, downloaded: 2, skipped: 0, save_path: '/tmp/downloads/테스트 디시콘_123' }

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason: string) => void
  const promise = new Promise<T>((res, rej) => { resolve = res; reject = rej })
  return { promise, resolve, reject }
}

async function lookupAndSelectPath() {
  fireEvent.change(screen.getByLabelText('디시콘 주소 또는 번호'), { target: { value: '123' } })
  fireEvent.click(screen.getByRole('button', { name: '조회' }))
  await screen.findByRole('heading', { name: packageInfo.title })
  fireEvent.click(screen.getByRole('button', { name: '폴더 선택' }))
  await waitFor(() => expect((screen.getByRole('textbox', { name: '저장 위치' }) as HTMLInputElement).value).toBe('/tmp/downloads'))
}

beforeEach(() => {
  backend.channels.length = 0
  backend.invoke.mockReset()
  backend.open.mockReset().mockResolvedValue('/tmp/downloads')
  backend.invoke.mockImplementation(async (command: string) => {
    if (command === 'parse_dccon_url') return '123'
    if (command === 'fetch_dccon_info') return packageInfo
    if (command === 'download_dccon') return completed
  })
})
afterEach(cleanup)

describe('download workflow', () => {
  it('requires a lookup and save location before downloading', async () => {
    render(<App />)
    expect((screen.getByRole('button', { name: '다운로드' }) as HTMLButtonElement).disabled).toBe(true)
    await lookupAndSelectPath()
    expect(screen.getByText('GIF, PNG')).toBeTruthy()
    expect((screen.getByRole('button', { name: '다운로드' }) as HTMLButtonElement).disabled).toBe(false)
    fireEvent.click(screen.getByRole('button', { name: '다운로드' }))
    await screen.findByText('다운로드를 완료했습니다')
    expect(screen.getByText(completed.save_path)).toBeTruthy()
    expect(screen.getByText('100%')).toBeTruthy()
    expect(backend.invoke.mock.calls.filter(([command]) => command === 'fetch_dccon_info')).toHaveLength(1)
    expect(backend.invoke.mock.calls.find(([command]) => command === 'download_dccon')?.[1]).toMatchObject({ packageIdx: '123', savePath: '/tmp/downloads' })
  })

  it('prevents duplicate lookups while a request is pending', async () => {
    const pending = deferred<typeof packageInfo>()
    backend.invoke.mockImplementation(async (command: string) => command === 'parse_dccon_url' ? '123' : pending.promise)
    render(<App />)
    fireEvent.change(screen.getByLabelText('디시콘 주소 또는 번호'), { target: { value: '123' } })
    const form = screen.getByLabelText('디시콘 주소 또는 번호').closest('form')!
    fireEvent.submit(form)
    fireEvent.submit(form)
    await waitFor(() => expect(backend.invoke).toHaveBeenCalledWith('fetch_dccon_info', { packageIdx: '123' }))
    expect(backend.invoke.mock.calls.filter(([command]) => command === 'parse_dccon_url')).toHaveLength(1)
    await act(async () => pending.resolve(packageInfo))
  })

  it('cancels the current download and lets the user resume completed files', async () => {
    const pending = deferred<typeof completed>()
    render(<App />)
    await lookupAndSelectPath()
    backend.invoke.mockImplementation((command: string) => command === 'download_dccon' ? pending.promise : Promise.resolve())
    fireEvent.click(screen.getByRole('button', { name: '다운로드' }))
    expect((screen.getByRole('button', { name: '중지' }) as HTMLButtonElement).disabled).toBe(true)
    await act(async () => backend.channels[0].onmessage({ current: 1, total: 2, downloaded: 1, skipped: 0, filename: '0.gif', status: '다운로드 중' }))
    expect(screen.getByText('50%')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: '중지' }))
    const downloadArgs = backend.invoke.mock.calls.find(([command]) => command === 'download_dccon')?.[1]
    await waitFor(() => expect(backend.invoke).toHaveBeenCalledWith('cancel_download', { downloadId: downloadArgs.downloadId }))
    await act(async () => pending.resolve({ ...completed, cancelled: true, downloaded: 1 }))
    await screen.findByText('다운로드를 중지했습니다')
    expect(screen.getByRole('button', { name: '이어서 다운로드' })).toBeTruthy()
    backend.invoke.mockResolvedValue({ ...completed, downloaded: 1, skipped: 1 })
    fireEvent.click(screen.getByRole('button', { name: '이어서 다운로드' }))
    await screen.findByText('다운로드를 완료했습니다')
    expect(screen.getByText('새로 저장 1개 · 기존 파일 1개')).toBeTruthy()
  })

  it('keeps completed progress visible after failure and ignores late channel messages', async () => {
    const pending = deferred<typeof completed>()
    render(<App />)
    await lookupAndSelectPath()
    backend.invoke.mockReturnValue(pending.promise)
    fireEvent.click(screen.getByRole('button', { name: '다운로드' }))
    await act(async () => backend.channels[0].onmessage({ current: 1, total: 2, downloaded: 1, skipped: 0, filename: '0.gif', status: '다운로드 중' }))
    await act(async () => pending.reject('HTTP 429. 자동 재시도 없이 중단했습니다.'))
    expect(screen.getByRole('alert').textContent).toContain('작업을 완료하지 못했습니다')
    expect(screen.getByText('50%')).toBeTruthy()
    expect(screen.getByRole('button', { name: '이어서 다운로드' })).toBeTruthy()
    await act(async () => backend.channels[0].onmessage({ current: 2, total: 2, downloaded: 2, skipped: 0, filename: '', status: '다운로드 완료' }))
    expect(screen.queryByText('100%')).toBeNull()
  })
})
