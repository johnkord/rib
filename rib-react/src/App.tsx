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
import { AuthProvider } from './hooks/useAuth';
import './main.css';

export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <AuthProvider>
          <div className="min-h-screen bg-gray-50 flex flex-col">
            <Navbar />
            <div className="container mx-auto px-4 flex-1 w-full">
              <Routes>
                <Route path="/" element={<BoardsPage />} />
                <Route path="/:slug" element={<BoardThreadsPage />} />
                <Route path="/thread/:id" element={<ThreadPage />} />
                <Route path="/admin/roles" element={<AdminRoles />} />
                <Route path="/login" element={<LoginPage />} />
                <Route path="/about" element={<About />} />
              </Routes>
            </div>
            <Footer />
          </div>
        </AuthProvider>
      </BrowserRouter>
    </QueryClientProvider>
  );
}
