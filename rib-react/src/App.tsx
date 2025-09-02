import { QueryClientProvider } from '@tanstack/react-query';
import { queryClient } from './lib/api';
import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { Navbar } from './components/Navbar';
import { BoardsPage } from './pages/BoardsPage';
import { BoardThreadsPage } from './pages/BoardThreadsPage';
import { ThreadPage } from './pages/ThreadPage';
import './main.css';

export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <Navbar />
        <div className="container mx-auto px-4">
          <Routes>
            <Route path="/" element={<BoardsPage />} />
            <Route path="/b/:slug" element={<BoardThreadsPage />} />
            <Route path="/thread/:id" element={<ThreadPage />} />
          </Routes>
        </div>
      </BrowserRouter>
    </QueryClientProvider>
  );
}
