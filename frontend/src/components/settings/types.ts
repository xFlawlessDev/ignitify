export type CertificateProvider = "none" | "lets-encrypt" | "custom";

export interface CustomCertificateSummary {
  id: string;
  name: string;
  certificateFileName: string;
  privateKeyFileName: string;
}
