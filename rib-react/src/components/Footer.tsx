import { Link } from 'react-router-dom';

export function Footer() {
  return (
    <footer className="mt-12 mb-4 text-center text-xs text-gray-500 flex flex-col items-center gap-1">
      <div className="opacity-80">© {new Date().getFullYear()} rib</div>
      <div className="flex items-center gap-3">
        <Link to="/about" className="link link-hover">
          About
        </Link>
        <a
          href="https://github.com/johnkord/rib"
          className="link link-hover"
          target="_blank"
          rel="noopener noreferrer"
        >
          GitHub
        </a>
      </div>
    </footer>
  );
}
