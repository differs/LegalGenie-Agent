import { createContext, useCallback, useContext, useEffect, useState } from 'react'
import { HashRouter, Navigate, Route, Routes } from 'react-router-dom'
import type { Session } from './lib/types'
import { clearSession, loadSession, saveSession } from './lib/session'
import * as api from './lib/api'
import { AuthPage } from './auth/AuthPage'
import { Workbench } from './workspace/Workbench'

type AuthState =
  | { status: 'loading'; session: null }
  | { status: 'signed-out'; session: null }
  | { status: 'signed-in'; session: Session }

type AuthContextValue = {
  auth: AuthState
  token: string | null
  signIn: (session: Session) => void
  signOut: () => void
}

const AuthContext = createContext<AuthContextValue | null>(null)

export function useAuth(): AuthContextValue {
  const ctx = useContext(AuthContext)
  if (!ctx) throw new Error('useAuth must be used within AuthProvider')
  return ctx
}

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [auth, setAuth] = useState<AuthState>({ status: 'loading', session: null })

  useEffect(() => {
    const existing = loadSession()
    if (!existing) {
      setAuth({ status: 'signed-out', session: null })
      return
    }

    // Try to refresh the session on boot; if that fails, keep the cached one
    // (the first authenticated request will surface 401 and sign us out).
    api
      .refreshToken(existing.refreshToken)
      .then((session) => {
        saveSession(session)
        setAuth({ status: 'signed-in', session })
      })
      .catch(() => {
        setAuth({ status: 'signed-in', session: existing })
      })
  }, [])

  const signIn = useCallback((session: Session) => {
    saveSession(session)
    setAuth({ status: 'signed-in', session })
  }, [])

  const signOut = useCallback(() => {
    clearSession()
    setAuth({ status: 'signed-out', session: null })
  }, [])

  const token = auth.session?.accessToken ?? null

  return (
    <AuthContext.Provider value={{ auth, token, signIn, signOut }}>
      {children}
    </AuthContext.Provider>
  )
}

export function App() {
  return (
    <AuthProvider>
      <HashRouter>
        <Routes>
          <Route path="/login" element={<AuthPage />} />
          <Route path="/" element={<Workbench />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </HashRouter>
    </AuthProvider>
  )
}
