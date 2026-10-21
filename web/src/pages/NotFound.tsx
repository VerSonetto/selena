import { Link } from "react-router-dom";
import { EmptyState } from "../components/ui";

export function NotFound() {
  return (
    <div className="page">
      <EmptyState
        title="页面不存在"
        description="请检查地址，或返回概览。"
        action={<Link className="btn btn-primary btn-md" to="/">返回概览</Link>}
      />
    </div>
  );
}
