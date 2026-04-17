import StatCards from "../components/dashboard/StatCards";
import ProjectOverview from "../components/dashboard/ProjectOverview";
import RecentActivity from "../components/dashboard/RecentActivity";
import MilestonesPanel from "../components/dashboard/MilestonesPanel";
import TeamWorkload from "../components/dashboard/TeamWorkload";
import UpcomingReviews from "../components/dashboard/UpcomingReviews";
import BudgetStatus from "../components/dashboard/BudgetStatus";
import ApprovalsQueue from "../components/dashboard/ApprovalsQueue";

interface DashboardProps {
  onNavigate?: (section: string) => void;
}

export default function Dashboard({ onNavigate = () => {} }: DashboardProps) {
  return (
    <div data-cmp="Dashboard" className="flex-1 overflow-y-auto scrollbar-thin bg-background">
      <div className="max-w-[1200px] mx-auto px-6 py-5">
        {/* Stat Cards */}
        <StatCards />

        {/* Hero Row — Projects + Activity */}
        <div className="flex gap-4 mt-4">
          {/* Projects — wide */}
          <div className="flex-1 min-w-0">
            <ProjectOverview onSelectProject={(id) => {
              console.log("navigate to project", id);
              onNavigate("projects");
            }} />
          </div>
          {/* Activity — narrow */}
          <div className="w-72 shrink-0">
            <RecentActivity />
          </div>
        </div>

        {/* Bottom Row */}
        <div className="flex gap-4 mt-4">
          {/* Budget Chart */}
          <div className="flex-1 min-w-0">
            <BudgetStatus />
          </div>
          {/* Milestones */}
          <div className="w-60 shrink-0">
            <MilestonesPanel />
          </div>
        </div>

        {/* Bottom 3-col Row */}
        <div className="flex gap-4 mt-4">
          <div className="flex-1 min-w-0">
            <UpcomingReviews />
          </div>
          <div className="flex-1 min-w-0">
            <ApprovalsQueue />
          </div>
          <div className="flex-1 min-w-0">
            <TeamWorkload />
          </div>
        </div>

        {/* Spacer */}
        <div className="h-6" />
      </div>
    </div>
  );
}
