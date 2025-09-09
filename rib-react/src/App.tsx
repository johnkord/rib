import { QueryClientProvider } from '@tanstack/react-query';
import { queryClient } from './lib/api';
import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { Navbar } from './components/Navbar';
import { Footer } from './components/Footer';
import { BoardsPage } from './pages/BoardsPage';
import { BoardThreadsPage } from './pages/BoardThreadsPage';
import { ThreadPage } from './pages/ThreadPage';
import { AdminRoles } from './pages/AdminRoles';
import { About } from './pages/About';
import { LoginPage } from './pages/LoginPage';
import { useEffect } from 'react';
import { setAuthToken, getAuthToken } from './lib/auth';
import './main.css';

export function App() {
  // Global token catcher (works for callback landing on any route)
  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const token = params.get('token');
    if (token && token !== getAuthToken()) {
      setAuthToken(token);
    }
    if (token) {
      params.delete('token');
      const newQs = params.toString();
      const newUrl = window.location.pathname + (newQs ? `?${newQs}` : '') + window.location.hash;
      window.history.replaceState(null, '', newUrl);
    }
  }, []);
  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <div className="min-h-screen bg-gray-50 flex flex-col">
          <Navbar />
          <div className="container mx-auto px-4 flex-1 w-full">
            <Routes>
              <Route path="/" element={<BoardsPage />} />
              <Route path="/:slug" element={<BoardThreadsPage />} />
              <Route path="/thread/:id" element={<ThreadPage />} />
              <Route path="/admin/roles" element={<AdminRoles />} />
              <Route path="/login" element={<LoginPage />} /> {/* NEW */}
              <Route path="/about" element={<About />} />
            </Routes>
          </div>
          <Footer />
        </div>
      </BrowserRouter>
    </QueryClientProvider>
  );
}
