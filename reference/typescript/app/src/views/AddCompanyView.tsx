// Standalone "add company" route — the same form as onboarding, reached from
// the portfolio when the workspace is already populated.

import { Link, useNavigate } from "react-router-dom";
import { CompanyForm } from "../components/CompanyForm";

export function AddCompanyView() {
  const navigate = useNavigate();
  return (
    <section>
      <div className="page-head">
        <div>
          <h2>Tilføj virksomhed</h2>
          <p className="muted">
            Registrerer en ny virksomhed i arbejdsområdet og opretter dens
            regnskab.
          </p>
        </div>
        <Link className="btn secondary" to="/">
          Annullér
        </Link>
      </div>
      <CompanyForm onCreated={(slug) => navigate(`/companies/${slug}`)} />
    </section>
  );
}
