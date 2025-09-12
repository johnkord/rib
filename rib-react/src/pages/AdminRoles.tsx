import { useState, useEffect } from 'react';
import { useAuth } from '../hooks/useAuth';
import { postJson } from '../lib/api';

export function AdminRoles() {
  const { user } = useAuth();
  const [subject, setSubject] = useState(''); // e.g. discord:123 or btc:addr
  const [selectedRole, setSelectedRole] = useState('user');
  const [message, setMessage] = useState('');
  const [error, setError] = useState('');
  const [roles, setRoles] = useState<Array<{ subject: string; role: string }>>([]);
  const [loading, setLoading] = useState(false);

  async function loadRoles() {
    setLoading(true);
    try {
      const res = await fetch('/api/v1/admin/roles', {
        headers: { Authorization: `Bearer ${localStorage.getItem('rib_auth_token')}` },
      });
      if (res.ok) {
        setRoles(await res.json());
      } else {
        setError(await res.text());
      }
    } catch (e: any) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    if (user?.role === 'admin') {
      loadRoles();
    }
  }, [user]);

  // Only admins can access this page
  if (!user || user.role !== 'admin') {
    return (
      <div className="container mx-auto p-4">
        <h1 className="text-2xl font-bold text-red-500">Access Denied</h1>
        <p>This page is only accessible to administrators.</p>
      </div>
    );
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setMessage('');
    setError('');

    try {
      await postJson('/admin/roles', { subject: subject.trim(), role: selectedRole });
      setMessage(`Role for ${subject.trim()} set to ${selectedRole}`);
      setSubject('');
      setSelectedRole('user');
      loadRoles();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update role');
    }
  };

  const deleteRole = async (subj: string) => {
    if (!confirm(`Delete role assignment for ${subj}?`)) return;
    try {
      const res = await fetch(`/api/v1/admin/roles/${encodeURIComponent(subj)}`, {
        method: 'DELETE',
        headers: { Authorization: `Bearer ${localStorage.getItem('rib_auth_token')}` },
      });
      if (res.status === 204) {
        setMessage(`Deleted ${subj}`);
        loadRoles();
      } else {
        setError(await res.text());
      }
    } catch (e: any) {
      setError(e.message);
    }
  };

  return (
    <div className="container mx-auto p-4">
      <h1 className="text-2xl font-bold mb-6">Role Management</h1>

      <div className="max-w-md">
        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label htmlFor="subject" className="block text-sm font-medium mb-1">
              Subject Key
            </label>
            <input
              id="subject"
              type="text"
              value={subject}
              onChange={(e) => setSubject(e.target.value)}
              placeholder="discord:123456789012345678 or btc:bc1q..."
              className="w-full p-2 border rounded font-mono text-xs"
              required
            />
            <p className="text-xs text-gray-500 mt-1 space-y-1">
              <span>Format: &lt;provider&gt;:&lt;identifier&gt;</span>
              <br />
              <span>Examples: discord:123456789012345678, btc:1A1zP1...</span>
            </p>
          </div>

          <div>
            <label htmlFor="role" className="block text-sm font-medium mb-1">
              Role
            </label>
            <select
              id="role"
              value={selectedRole}
              onChange={(e) => setSelectedRole(e.target.value)}
              className="w-full p-2 border rounded"
            >
              <option value="user">User</option>
              <option value="moderator">Moderator</option>
              <option value="admin">Admin</option>
            </select>
          </div>

          <button
            type="submit"
            className="w-full bg-blue-500 text-white py-2 px-4 rounded hover:bg-blue-600"
          >
            Update Role
          </button>
        </form>

        {message && <div className="mt-4 p-3 bg-green-100 text-green-700 rounded">{message}</div>}

        {error && <div className="mt-4 p-3 bg-red-100 text-red-700 rounded">{error}</div>}
      </div>

      <div className="mt-8 p-4 bg-gray-100 rounded">
        <h2 className="font-semibold mb-2">How it works:</h2>
        <ul className="text-sm space-y-1">
          <li>• Assign roles to any auth subject (Discord, Bitcoin, future providers)</li>
          <li>• Subject must match the JWT sub prefix pattern used during login</li>
          <li>• Roles: user (default), moderator, admin</li>
        </ul>
      </div>

      <div className="mt-10">
        <div className="flex items-center justify-between mb-2">
          <h2 className="font-semibold">Existing Assignments</h2>
          <button className="btn btn-xs" onClick={loadRoles} disabled={loading}>
            {loading ? 'Refreshing...' : 'Refresh'}
          </button>
        </div>
        {roles.length === 0 && <div className="text-xs text-gray-500">No assignments yet.</div>}
        <div className="overflow-x-auto">
          <table className="table table-zebra table-xs w-full">
            <thead>
              <tr>
                <th>Subject</th>
                <th>Role</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {roles.map((r) => (
                <tr key={r.subject}>
                  <td className="font-mono text-[11px]">{r.subject}</td>
                  <td>{r.role}</td>
                  <td className="space-x-2">
                    <button
                      className="btn btn-ghost btn-xs"
                      onClick={() => {
                        setSubject(r.subject);
                        setSelectedRole(r.role);
                        window.scrollTo({ top: 0, behavior: 'smooth' });
                      }}
                    >
                      Edit
                    </button>
                    <button className="btn btn-error btn-xs" onClick={() => deleteRole(r.subject)}>
                      Delete
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
