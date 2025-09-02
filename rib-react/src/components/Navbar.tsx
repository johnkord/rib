import { Link } from 'react-router-dom';

export function Navbar() {
  return (
    <div className="navbar bg-base-200 mb-4 px-4">
      <div className="flex-1">
        <Link to="/" className="text-xl font-bold">rib</Link>
      </div>
      <div className="flex-none">
        <Link to="/" className="btn btn-ghost">Boards</Link>
      </div>
    </div>
  );
}
