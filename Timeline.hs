module Timeline
  ( Suspect
  , TimeRange(..)
  , Alibi(..)
  , Evidence(..)
  , DAG
  , buildDAG
  , canCommit
  , readFromJSON
  , dagVisualize
  ) where

import Control.Monad (guard)
import Data.Aeson (FromJSON(..), eitherDecode, withObject, (.:), (.:?))
import Data.ByteString.Lazy.Char8 (pack)
import Data.Either (either)
import Data.Graph (Graph, Vertex, graphFromEdges, vertices)
import Data.List (foldl', intercalate)
import qualified Data.Map.Strict as Map

type Suspect = String

data TimeRange = TimeRange
  { startTime :: Int
  , endTime :: Int
  } deriving (Eq, Ord, Show)

data Alibi = Alibi
  { alibiPerson :: Suspect
  , alibiRange :: TimeRange
  , alibiWitnesses :: [String]
  , alibiConfidence :: Double
  } deriving (Eq, Show)

data Evidence = Evidence
  { evidenceDescription :: String
  , evidenceTimestamp :: Int
  , evidenceReliability :: Double
  , evidenceSuspectInvolved :: Suspect
  } deriving (Eq, Show)

data TimelinePayload = TimelinePayload
  { payloadAlibis :: [Alibi]
  , payloadEvidence :: [Evidence]
  } deriving (Eq, Show)

instance FromJSON TimeRange where
  parseJSON = withObject "TimeRange" $ \o -> do
    start <- o .: "start"
    end <- o .: "end"
    guard (start <= end)
    pure (TimeRange start end)

instance FromJSON Alibi where
  parseJSON = withObject "Alibi" $ \o -> do
    person <- o .: "person"
    range <- o .: "timeRange"
    witnesses <- o .:? "witnesses" .!= []
    confidence <- o .:? "confidence" .!= 0
    pure (Alibi person range witnesses confidence)

instance FromJSON Evidence where
  parseJSON = withObject "Evidence" $ \o -> do
    description <- o .: "description"
    timestamp <- o .: "timestamp"
    reliability <- o .:? "reliability" .!= 0
    suspect <- o .: "suspectInvolved"
    pure (Evidence description timestamp reliability suspect)

instance FromJSON TimelinePayload where
  parseJSON = withObject "Timeline" $ \o -> do
    alibis <- o .:? "alibis" .!= []
    evidence <- o .:? "evidence" .!= []
    pure (TimelinePayload alibis evidence)

data NodeMeta = NodeMeta
  { nmSuspect :: Suspect
  , nmRange :: TimeRange
  , nmAlibiConfidence :: Maybe Double
  , nmWitnessReliability :: Maybe Double
  } deriving (Eq, Show)

type NodeKey = (Suspect, TimeRange)

data DAG = DAG
  { dagGraph :: Graph
  , dagVertexInfo :: Vertex -> (NodeMeta, NodeKey, [NodeKey])
  , dagLookup :: NodeKey -> Maybe Vertex
  }

readFromJSON :: String -> Either String ([Alibi], [Evidence])
readFromJSON input =
  either
    (Left . ("readFromJSON: " ++))
    (\payload -> Right (payloadAlibis payload, payloadEvidence payload))
    (eitherDecode (pack input))

buildDAG :: [Alibi] -> [Evidence] -> DAG
buildDAG alibis evidence =
  let nodeMap = foldl' insertAlibi Map.empty alibis
      nodeMap' = foldl' insertEvidence nodeMap evidence
      entries = map (entryFromNode nodeMap') (Map.assocs nodeMap')
      (graph, vertexInfo, lookupKey) = graphFromEdges entries
   in DAG graph vertexInfo lookupKey

canCommit :: Suspect -> TimeRange -> [Alibi] -> [Evidence] -> Maybe Bool
canCommit suspect range alibis evidence =
  let dag = buildDAG alibis evidence
      hasStrongEvidence = any strongWitness (filter (evidenceInRange range) evidence)
      matches v =
        let (node, _, _) = dagVertexInfo dag v
         in nmSuspect node == suspect && rangesOverlap (nmRange node) range
      hasStrongAlibi = any (nodeHasStrongAlibi dag) (filter matches (vertices (dagGraph dag)))
   in if hasStrongEvidence
        then Just True
        else if hasStrongAlibi
          then Just False
          else Nothing

dagVisualize :: DAG -> String
dagVisualize dag =
  let nodeLines = map (vertexLine dag) (vertices (dagGraph dag))
   in unlines ("nodes:" : nodeLines)

nodeHasStrongAlibi :: DAG -> Vertex -> Bool
nodeHasStrongAlibi dag vertex =
  let (node, _, succKeys) = dagVertexInfo dag vertex
      strong = maybe False (>= 0.75) (nmAlibiConfidence node)
      conflict = any (hasHighReliability dag) succKeys
   in strong && not conflict

hasHighReliability :: DAG -> NodeKey -> Bool
hasHighReliability dag key = case dagLookup dag key of
  Nothing -> False
  Just vertex ->
    let (node, _, _) = dagVertexInfo dag vertex
     in maybe False (>= 0.7) (nmWitnessReliability node)

vertexLine :: DAG -> Vertex -> String
vertexLine dag vertex =
  let (node, _, successors) = dagVertexInfo dag vertex
      label = showNodeMeta node
      succText = if null successors then "" else " -> " ++ intercalate ", " (map showNodeKey successors)
   in label ++ succText

showNodeMeta :: NodeMeta -> String
showNodeMeta node =
  nmSuspect node ++ "@" ++ showRange (nmRange node)

showNodeKey :: NodeKey -> String
showNodeKey (suspect, rng) = suspect ++ "@" ++ showRange rng

showRange :: TimeRange -> String
showRange rg = "[" ++ show (startTime rg) ++ "," ++ show (endTime rg) ++ "]"

rangesOverlap :: TimeRange -> TimeRange -> Bool
rangesOverlap a b = not (endTime a < startTime b || endTime b < startTime a)

evidenceInRange :: TimeRange -> Evidence -> Bool
evidenceInRange range evidence =
  let ts = evidenceTimestamp evidence
   in startTime range <= ts && ts <= endTime range

strongWitness :: Evidence -> Bool
strongWitness evidence = evidenceReliability evidence >= 0.75

insertAlibi :: Map.Map NodeKey NodeMeta -> Alibi -> Map.Map NodeKey NodeMeta
insertAlibi m alibi =
  Map.insertWith combineNodeMeta (keyFromAlibi alibi) (nodeFromAlibi alibi) m

insertEvidence :: Map.Map NodeKey NodeMeta -> Evidence -> Map.Map NodeKey NodeMeta
insertEvidence m evidence =
  Map.insertWith combineNodeMeta (keyFromEvidence evidence) (nodeFromEvidence evidence) m

nodeFromAlibi :: Alibi -> NodeMeta
nodeFromAlibi alibi =
  NodeMeta
    (alibiPerson alibi)
    (alibiRange alibi)
    (Just (alibiConfidence alibi))
    Nothing

nodeFromEvidence :: Evidence -> NodeMeta
nodeFromEvidence evidence =
  NodeMeta
    (evidenceSuspectInvolved evidence)
    (evidenceRange evidence)
    Nothing
    (Just (evidenceReliability evidence))

keyFromAlibi :: Alibi -> NodeKey
keyFromAlibi alibi = (alibiPerson alibi, alibiRange alibi)

keyFromEvidence :: Evidence -> NodeKey
keyFromEvidence evidence = (evidenceSuspectInvolved evidence, evidenceRange evidence)

evidenceRange :: Evidence -> TimeRange
evidenceRange evidence =
  let ts = evidenceTimestamp evidence
   in TimeRange ts (ts + 1)

combineNodeMeta :: NodeMeta -> NodeMeta -> NodeMeta
combineNodeMeta new old =
  NodeMeta
    (nmSuspect new)
    (nmRange new)
    (maxMaybe (nmAlibiConfidence new) (nmAlibiConfidence old))
    (maxMaybe (nmWitnessReliability new) (nmWitnessReliability old))

entryFromNode :: Map.Map NodeKey NodeMeta -> (NodeKey, NodeMeta) -> (NodeMeta, NodeKey, [NodeKey])
entryFromNode nodeMap (key, node) =
  (node, key, conflictTargets nodeMap node)

conflictTargets :: Map.Map NodeKey NodeMeta -> NodeMeta -> [NodeKey]
conflictTargets nodeMap node =
  case nmAlibiConfidence node of
    Nothing -> []
    Just _ ->
      [ key
      | (key, candidate) <- Map.assocs nodeMap
      , nmSuspect candidate /= nmSuspect node
      , maybe False (> 0.5) (nmWitnessReliability candidate)
      , rangesOverlap (nmRange node) (nmRange candidate)
      ]

maxMaybe :: Maybe Double -> Maybe Double -> Maybe Double
maxMaybe Nothing y = y
maxMaybe x Nothing = x
maxMaybe (Just x) (Just y) = Just (max x y)
