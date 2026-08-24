import { create } from "zustand";

export type SessionUser = { id: string; username: string; email?: string | null };

type SessionState = {
  token: string | null;
  user: SessionUser | null;
  setSession: (token: string, user: SessionUser) => void;
  clearSession: () => void;
};

const TOKEN_KEY = "opencade.session_token";

function storedToken(): string | null {
  try {
    return localStorage.getItem(TOKEN_KEY);
  } catch {
    return null;
  }
}

export const useSessionStore = create<SessionState>((set) => ({
  token: storedToken(),
  user: null,
  setSession: (token, user) => {
    localStorage.setItem(TOKEN_KEY, token);
    set({ token, user });
  },
  clearSession: () => {
    localStorage.removeItem(TOKEN_KEY);
    set({ token: null, user: null });
  },
}));
