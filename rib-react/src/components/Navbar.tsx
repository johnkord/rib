import { Link } from 'react-router-dom';
import { useAuth } from '../hooks/useAuth';

export function Navbar() {
  const { user, logout } = useAuth();

  return (
    <div className="navbar bg-base-200 mb-4 px-4">
      <div className="flex-1">
        <Link to="/" className="text-xl font-bold">rib</Link>
      </div>
      <div className="flex-none">
  <Link to="/" className="btn btn-ghost">Boards</Link>
        {user ? (
          <div className="flex items-center space-x-4">
            <span className="text-sm text-gray-600">
              {user.username} ({user.role})
            </span>
            {user.role === 'admin' && (
              <Link
                to="/admin/roles"
                className="text-sm text-blue-600 hover:text-blue-800"
              >
                Manage Roles
              </Link>
            )}
            <button
              onClick={logout}
              className="text-sm text-red-600 hover:text-red-800"
            >
              Logout
            </button>
          </div>
        ) : (
          <Link to="/login" className="btn btn-primary btn-sm">Login</Link>
        )}
      </div>
    </div>
  );
}
