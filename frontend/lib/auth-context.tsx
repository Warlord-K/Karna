'use client';

import { createContext, useContext } from 'react';

const AuthDisabledContext = createContext(false);

export function AuthDisabledProvider({ value, children }: { value: boolean; children: React.ReactNode }) {
  return (
    <AuthDisabledContext.Provider value={value}>
      {children}
    </AuthDisabledContext.Provider>
  );
}

export function useAuthDisabled() {
  return useContext(AuthDisabledContext);
}
